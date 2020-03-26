use crate::filters::Filter;
use config;
use filters;
use itertools::Itertools;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FFT;

// Sample format
use PrcFmt;
use Res;

pub struct FFTConv {
    name: String,
    npoints: usize,
    nsegments: usize,
    overlap: Vec<PrcFmt>,
    coeffs_f: Vec<Vec<Complex<PrcFmt>>>,
    coeffs_k1: Vec<Vec<Complex<PrcFmt>>>,
    coeffs_k2: Vec<Vec<Complex<PrcFmt>>>,
    fft: Box<dyn rustfft::FFT<PrcFmt>>,
    ifft: Box<dyn rustfft::FFT<PrcFmt>>,
    input_buf: Vec<Complex<PrcFmt>>,
    input_f: Vec<Vec<Complex<PrcFmt>>>,
    temp_buf: Vec<Complex<PrcFmt>>,
    output_buf: Vec<Complex<PrcFmt>>,
    index: usize,
}

impl FFTConv {
    //fn pack_complex(real_data: &[PrcFmt], complex_data)

    /// Create a new FFT colvolution filter.
    pub fn new(name: String, data_length: usize, coeffs: &[PrcFmt]) -> Self {
        let input_buf: Vec<Complex<PrcFmt>> = vec![Complex::zero(); data_length];
        let temp_buf: Vec<Complex<PrcFmt>> = vec![Complex::zero(); data_length];
        let output_buf: Vec<Complex<PrcFmt>> = vec![Complex::zero(); data_length];
        let fft = Radix4::new(data_length, false);
        let ifft = Radix4::new(data_length, true);
        
        let fft_temp = Radix4::new(2*data_length, false);


        let nsegments = ((coeffs.len() as PrcFmt) / (data_length as PrcFmt)).ceil() as usize;
        println!("segments: {}", nsegments);
        let input_f = vec![vec![Complex::zero(); data_length]; nsegments];
        let mut coeffs_f = vec![vec![Complex::zero(); 2*data_length]; nsegments];
        let mut coeffs_k1 = vec![vec![Complex::zero(); data_length]; nsegments];
        let mut coeffs_k2 = vec![vec![Complex::zero(); data_length]; nsegments];
        let mut coeffs_c = vec![vec![Complex::zero(); 2*data_length]; nsegments];

        debug!("Conv {} is using {} segments", name, nsegments);
        //let mut n = 0;
        //for pair in (coeffs.iter().chunks(2)).into_iter().map(|mut chunk| Complex::new(*chunk.next().unwrap_or(&0.0), *chunk.next().unwrap_or(&0.0))){
        //    coeffs_c[n / data_length][n % data_length] =
        //        pair.unscale((data_length) as PrcFmt);
        //        coeffs_segm[]
        //    n+=1;
        //    //println!("{}, {}", n, pair);
        //}
        for (n, coeff) in coeffs.iter().enumerate() {
            coeffs_c[n / data_length][n % data_length] =
                Complex::from(coeff / (data_length as PrcFmt));
        }

        //for (segment, segment_f) in coeffs_c.iter_mut().zip(coeffs_f.iter_mut()) {
        //    fft.process(segment, segment_f);
        //}
        println!("coeffs_c: {:?}", coeffs_c);
        let mut m = 0;
        for (segment, segment_f) in coeffs_c.iter_mut().zip(coeffs_f.iter_mut()) {
            fft_temp.process(segment, segment_f);
            for n in 0..data_length {
                //println!("n {}", n);
                let nfracpi = (std::f64::consts::PI as PrcFmt)*(n as PrcFmt)/(data_length as PrcFmt);
                coeffs_k1[m][n] = (segment_f[n] - segment_f[(data_length - n)].conj()).scale(0.5*(nfracpi.cos()));
                coeffs_k2[m][n] = (segment_f[n].scale(1.0-nfracpi.sin()) + segment_f[(data_length - n)].conj().scale(1.0+nfracpi.sin())).scale(0.5);
                //coeffs_k1[m][n] = (segment_f[n] - segment_f[(data_length - n)%data_length].conj()).scale(0.5*(nfracpi.cos()));
                //coeffs_k2[m][n] = (segment_f[n].scale(1.0-nfracpi.sin()) + segment_f[(data_length - n)%data_length].conj().scale(1.0+nfracpi.sin())).scale(0.5);
            }
            println!("k1[{}]: {:?}", m, coeffs_k1[m]);
            println!("k2[{}]: {:?}", m, coeffs_k2[m]);
            m+=1;
        }

        FFTConv {
            name,
            npoints: data_length,
            nsegments,
            overlap: vec![0.0; data_length],
            coeffs_f,
            coeffs_k1,
            coeffs_k2,
            fft: Box::new(fft),
            ifft: Box::new(ifft),
            input_f,
            input_buf,
            output_buf,
            temp_buf,
            index: 0,
        }
    }

    pub fn from_config(name: String, data_length: usize, conf: config::ConvParameters) -> Self {
        let values = match conf {
            config::ConvParameters::Values { values } => values,
            config::ConvParameters::File { filename, format } => {
                filters::read_coeff_file(&filename, &format).unwrap()
            }
        };
        FFTConv::new(name, data_length, &values)
    }
}

impl Filter for FFTConv {
    fn name(&self) -> String {
        self.name.clone()
    }

    /// Process a waveform by FT, then multiply transform with transform of filter, and then transform back.
    fn process_waveform(&mut self, waveform: &mut Vec<PrcFmt>) -> Res<()> {
        // Copy to inut buffer and convert to complex
        //for (n, item) in waveform.iter_mut().enumerate().take(self.npoints) {
        //    self.input_buf[n] = Complex::<PrcFmt>::from(*item);
        //    //self.input_buf[n+self.npoints] = Complex::zero();
        //}
        let mut n = 0;
        for pair in (waveform.iter().chunks(2)).into_iter().map(|mut chunk| Complex::new(*chunk.next().unwrap_or(&0.0), *chunk.next().unwrap_or(&0.0))){
            self.input_buf[n] = pair;
            n+=1;
            println!("packing.. {}, {}", n, pair);
        }
        println!("input: {:?}", self.input_buf);

        // FFT and store result in history, update index
        self.index = (self.index + 1) % self.nsegments;
        self.fft
            .process(&mut self.input_buf, &mut self.input_f[self.index]);
        println!("input fft: {:?}", self.input_f);
        //print!("a");
        //self.temp_buf = vec![Complex::zero(); 2 * self.npoints];
        // Loop through history of input FTs, multiply with filter FTs, accumulate result
        let segm = 0;
        let hist_idx = (self.index + self.nsegments - segm) % self.nsegments;
        for n in 0..self.npoints {
            //println!("s 0: {}", n);
            self.temp_buf[n] = -self.input_f[hist_idx][n] * self.coeffs_k1[segm][n]*Complex::i()
                      + self.input_f[hist_idx][(self.npoints - n)%self.npoints].conj() * self.coeffs_k2[segm][n];
        }
        for segm in 1..self.nsegments {
            let hist_idx = (self.index + self.nsegments - segm) % self.nsegments;
            for n in 0..self.npoints/2 {
                //println!("s {}: {}", segm, n);
                self.temp_buf[n] += -self.input_f[hist_idx][n] * self.coeffs_k1[segm][n]*Complex::i()
                          + self.input_f[hist_idx][(self.npoints - n)%self.npoints].conj() * self.coeffs_k2[segm][n];
            }
        }
        println!("multiplied: {:?}", self.temp_buf);
        //self.temp_buf = vec![Complex::new(0.0, -0.25); 4];
        // IFFT result, store result anv overlap
        self.ifft.process(&mut self.temp_buf, &mut self.output_buf);
        //let mut filtered: Vec<PrcFmt> = vec![0.0; self.npoints];

        println!("output: {:?}", self.output_buf);
        for n in 0..(self.npoints/2) {
            println!("out {}", n);
            waveform[2*n] = self.output_buf[n].re + self.overlap[2*n];
            println!("a");
            waveform[2*n+1] = -self.output_buf[n].im + self.overlap[2*n+1];
            println!("b");
            self.overlap[2*n] = self.output_buf[n + self.npoints/2].re;
            println!("c: {}, {}", 2*n+1, 2*n + self.npoints/2);
            self.overlap[2*n+1] = -self.output_buf[n + self.npoints/2].im;
        }
        println!("output wf: {:?}", waveform);
        println!("overlap: {:?}", self.overlap);

        //for (n, item) in waveform.iter_mut().enumerate().take(self.npoints) {
        //    *item = self.output_buf[n].re + self.overlap[n];
        //    self.overlap[n] = self.output_buf[n + self.npoints].re;
        //}
        Ok(())
    }

    fn update_parameters(&mut self, conf: config::Filter) {
        if let config::Filter::Conv { parameters: conf } = conf {
            let coeffs = match conf {
                config::ConvParameters::Values { values } => values,
                config::ConvParameters::File { filename, format } => {
                    filters::read_coeff_file(&filename, &format).unwrap()
                }
            };

            let nsegments = ((coeffs.len() as PrcFmt) / (self.npoints as PrcFmt)).ceil() as usize;

            if nsegments == self.nsegments {
                // Same length, lets keep history
            } else {
                // length changed, clearing history
                self.nsegments = nsegments;
                let input_f = vec![vec![Complex::zero(); 2 * self.npoints]; nsegments];
                self.input_f = input_f;
            }

            let mut coeffs_f = vec![vec![Complex::zero(); 2 * self.npoints]; nsegments];
            let mut coeffs_c = vec![vec![Complex::zero(); 2 * self.npoints]; nsegments];

            debug!("conv using {} segments", nsegments);

            for (n, coeff) in coeffs.iter().enumerate() {
                coeffs_c[n / self.npoints][n % self.npoints] =
                    Complex::from(coeff / (2.0 * self.npoints as PrcFmt));
            }

            for (segment, segment_f) in coeffs_c.iter_mut().zip(coeffs_f.iter_mut()) {
                self.fft.process(segment, segment_f);
            }
            self.coeffs_f = coeffs_f;
        } else {
            // This should never happen unless there is a bug somewhere else
            panic!("Invalid config change!");
        }
    }
}

/// Validate a FFT convolution config.
pub fn validate_config(conf: &config::ConvParameters) -> Res<()> {
    match conf {
        config::ConvParameters::Values { .. } => Ok(()),
        config::ConvParameters::File { filename, format } => {
            let coeffs = filters::read_coeff_file(&filename, &format)?;
            if coeffs.is_empty() {
                return Err(Box::new(config::ConfigError::new(
                    "Conv coefficients are empty",
                )));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::PrcFmt;
    use config::ConvParameters;
    use fftconv::FFTConv;
    use filters::Filter;

    fn is_close(left: PrcFmt, right: PrcFmt, maxdiff: PrcFmt) -> bool {
        println!("{} - {}", left, right);
        (left - right).abs() < maxdiff
    }

    fn compare_waveforms(left: Vec<PrcFmt>, right: Vec<PrcFmt>, maxdiff: PrcFmt) -> bool {
        for (val_l, val_r) in left.iter().zip(right.iter()) {
            if !is_close(*val_l, *val_r, maxdiff) {
                return false;
            }
        }
        true
    }

    #[test]
    fn check_simplest() {
        let coeffs = vec![1.0];
        let conf = ConvParameters::Values { values: coeffs };
        let mut filter = FFTConv::from_config("test".to_string(), 4, conf);
        let mut wave1 = vec![1.0, 0.0, 0.0, 1.0];
        let expected = vec![1.0, 0.0, 0.0, 1.0];
        filter.process_waveform(&mut wave1).unwrap();
        assert!(compare_waveforms(wave1, expected, 1e-7));
    }

    //1 1,0; 1,0; 1,0; 1,0
    //2 0,1; 0,1; 0,1; 0,1;
    //3 1,0; 0,-1; -1,0; 0,1
    //4 0,1; 1,0; 0,-1; -1,0

    #[test]
    fn check_realonly() {
        let coeffs = vec![1.0, 0.0, 1.0];
        let conf = ConvParameters::Values { values: coeffs };
        let mut filter = FFTConv::from_config("test".to_string(), 8, conf);
        let mut wave1 = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let expected = vec![1.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0];
        filter.process_waveform(&mut wave1).unwrap();
        assert!(compare_waveforms(wave1, expected, 1e-7));
    }

    #[test]
    fn check_imagonly() {
        let coeffs = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let conf = ConvParameters::Values { values: coeffs };
        let mut filter = FFTConv::from_config("test".to_string(), 8, conf);
        let mut wave1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let expected = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        filter.process_waveform(&mut wave1).unwrap();
        assert!(compare_waveforms(wave1, expected, 1e-7));
    }

    #[test]
    fn check_single() {
        let coeffs = vec![0.5, 0.5];
        let conf = ConvParameters::Values { values: coeffs };
        let mut filter = FFTConv::from_config("test".to_string(), 8, conf);
        let mut wave1 = vec![1.0, 1.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0];
        let expected = vec![0.5, 1.0, 1.0, 0.5, 0.0, -0.5, -0.5, 0.0];
        filter.process_waveform(&mut wave1).unwrap();
        assert!(compare_waveforms(wave1, expected, 1e-7));
    }

    #[test]
    fn check_segmented() {
        let mut coeffs = Vec::<PrcFmt>::new();
        for m in 0..32 {
            coeffs.push(m as PrcFmt);
        }
        let mut filter = FFTConv::new("test".to_owned(), 8, &coeffs);
        let mut wave1 = vec![0.0 as PrcFmt; 8];
        let mut wave2 = vec![0.0 as PrcFmt; 8];
        let mut wave3 = vec![0.0 as PrcFmt; 8];
        let mut wave4 = vec![0.0 as PrcFmt; 8];
        let mut wave5 = vec![0.0 as PrcFmt; 8];

        wave1[0] = 1.0;
        filter.process_waveform(&mut wave1).unwrap();
        filter.process_waveform(&mut wave2).unwrap();
        filter.process_waveform(&mut wave3).unwrap();
        filter.process_waveform(&mut wave4).unwrap();
        filter.process_waveform(&mut wave5).unwrap();

        let exp1 = Vec::from(&coeffs[0..8]);
        let exp2 = Vec::from(&coeffs[8..16]);
        let exp3 = Vec::from(&coeffs[16..24]);
        let exp4 = Vec::from(&coeffs[24..32]);
        let exp5 = vec![0.0 as PrcFmt; 8];

        assert!(compare_waveforms(wave1, exp1, 1e-5));
        assert!(compare_waveforms(wave2, exp2, 1e-5));
        assert!(compare_waveforms(wave3, exp3, 1e-5));
        assert!(compare_waveforms(wave4, exp4, 1e-5));
        assert!(compare_waveforms(wave5, exp5, 1e-5));
    }
}
