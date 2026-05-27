#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// base pointer to the bottom of the frame in the stack
    pub base: usize,

    /// source information (file, start, end)
    pub span: (u32, u32, u32),

    /// function index in the function table
    /// None if this frame is not associated with a function
    /// (e.g., the top-level frame, or a frame created by a direct call)
    pub function: Option<usize>,
}
