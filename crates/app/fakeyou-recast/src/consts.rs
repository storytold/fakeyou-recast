pub const MODEL_SAMPLE_RATE: usize = 16000;
pub const OFFLINE_CHUNK_SIZE: usize = 6*MODEL_SAMPLE_RATE;
pub const TENSOR_SIZE: usize = 32000; // 2 seconds at 16KHz
pub const VAD_SIZE: usize = 600; 
pub const WINDOW_WIDTH: u32 = 609;
pub const WINDOW_HEIGHT: u32 = 636;
pub const SPEAKER_THRESHOLD: f32 = 0.4;
