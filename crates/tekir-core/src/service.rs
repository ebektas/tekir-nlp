use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextRequest<Options> {
    pub input: String,
    pub options: Options,
}

impl<Options> TextRequest<Options> {
    pub fn new(input: impl Into<String>, options: Options) -> Self {
        Self {
            input: input.into(),
            options,
        }
    }
}

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

pub trait TextTransformer {
    type Options;
    type Output;

    fn execute(&self, request: TextRequest<Self::Options>) -> TextTransformResponse<Self::Output>;
}
