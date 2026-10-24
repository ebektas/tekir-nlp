use serde::Serialize;

/// Response envelope shared by the text services: the original input, its
/// byte length, and the service-specific output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextTransformResponse<Output> {
    pub original: String,
    pub output: Output,
    pub input_len_bytes: usize,
}

impl<Output> TextTransformResponse<Output> {
    pub fn new(original: String, output: Output) -> Self {
        let input_len_bytes = original.len();

        Self {
            original,
            output,
            input_len_bytes,
        }
    }
}
