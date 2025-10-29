use serde::de::DeserializeOwned;

/// A deserialized item or an unknown structure (with coordinates) for upstream handling.
#[derive(Debug, Clone)]
pub enum ParsedOrUnknown<T> {
    Parsed(T),
    Unknown(ObjCoords),
}

// =============== Streaming JSON structure discovery ===============

/// Type of a JSON node found by the stream parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Object,
    Array,
}

/// Coordinates of a JSON structure within a larger text, including nested children.
#[derive(Debug, Clone)]
pub struct ObjCoords {
    pub start: usize,
    pub end: usize, // inclusive index of the closing bracket/brace
    pub kind: NodeType,
    pub children: Vec<ObjCoords>,
}

impl ObjCoords {
    pub fn new(start: usize, end: usize, kind: NodeType, children: Vec<ObjCoords>) -> Self {
        Self { start, end, kind, children }
    }
}

#[derive(Debug)]
struct Frame {
    start: usize,
    kind: NodeType,
    children: Vec<ObjCoords>,
}

/// Find all JSON object/array structures in the given text. Coordinates are byte indices.
pub fn find_json_structures(text: &str) -> Vec<ObjCoords> {
    let bytes = text.as_bytes();
    let mut results: Vec<ObjCoords> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();

    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match b {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' => stack.push(Frame { start: i, kind: NodeType::Object, children: Vec::new() }),
            b'[' => stack.push(Frame { start: i, kind: NodeType::Array, children: Vec::new() }),
            b'}' => {
                if let Some(frame) = stack.pop() {
                    if frame.kind == NodeType::Object {
                        let node = ObjCoords::new(frame.start, i, NodeType::Object, frame.children);
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            results.push(node);
                        }
                    }
                }
            }
            b']' => {
                if let Some(frame) = stack.pop() {
                    if frame.kind == NodeType::Array {
                        let node = ObjCoords::new(frame.start, i, NodeType::Array, frame.children);
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            results.push(node);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    results
}

/// Stateful incremental stream parser that can be fed chunks and yields closed root nodes per feed.
#[derive(Debug, Default)]
pub struct JsonStreamParser {
    stack: Vec<Frame>,
    in_string: bool,
    escape: bool,
    /// Absolute offset (bytes) from the beginning of the full stream to the start of current chunk
    offset: usize,
}

impl JsonStreamParser {
    pub fn new() -> Self { Self::default() }

    /// Feed a new chunk. Returns any fully-closed root nodes found in this chunk.
    pub fn feed(&mut self, chunk: &str) -> Vec<ObjCoords> {
        let bytes = chunk.as_bytes();
        let mut roots: Vec<ObjCoords> = Vec::new();

        for (i, &b) in bytes.iter().enumerate() {
            let idx = self.offset + i;

            if self.in_string {
                if self.escape {
                    self.escape = false;
                    continue;
                }
                match b {
                    b'\\' => self.escape = true,
                    b'"' => self.in_string = false,
                    _ => {}
                }
                continue;
            }

            match b {
                b'"' => self.in_string = true,
                b'{' => self.stack.push(Frame { start: idx, kind: NodeType::Object, children: Vec::new() }),
                b'[' => self.stack.push(Frame { start: idx, kind: NodeType::Array, children: Vec::new() }),
                b'}' => {
                    if let Some(frame) = self.stack.pop() {
                        if frame.kind == NodeType::Object {
                            let node = ObjCoords::new(frame.start, idx, NodeType::Object, frame.children);
                            if let Some(parent) = self.stack.last_mut() {
                                parent.children.push(node);
                            } else {
                                roots.push(node);
                            }
                        }
                    }
                }
                b']' => {
                    if let Some(frame) = self.stack.pop() {
                        if frame.kind == NodeType::Array {
                            let node = ObjCoords::new(frame.start, idx, NodeType::Array, frame.children);
                            if let Some(parent) = self.stack.last_mut() {
                                parent.children.push(node);
                            } else {
                                roots.push(node);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.offset += bytes.len();
        roots
    }
}

/// Attempt to deserialize a node; if it fails, recursively try children.
fn descend_deserialize<T: DeserializeOwned>(text: &str, node: &ObjCoords, out: &mut Vec<ParsedOrUnknown<T>>) {
    let slice_end = node.end + 1; // end is inclusive
    let candidate = &text[node.start..slice_end];
    if let Ok(parsed) = serde_json::from_str::<T>(candidate) {
        out.push(ParsedOrUnknown::Parsed(parsed));
        return; // success: do not attempt internals
    }
    // Try children
    let before_len = out.len();
    for child in &node.children {
        descend_deserialize::<T>(text, child, out);
    }
    // If none of the children produced anything, surface this unknown
    if out.len() == before_len {
        out.push(ParsedOrUnknown::Unknown(node.clone()));
    }
}

/// Produce a flat stream of parsed items or unknown structures from the given text.
pub fn deserialize_stream_map<T: DeserializeOwned>(text: &str) -> Vec<ParsedOrUnknown<T>> {
    let mut out = Vec::new();
    let roots = find_json_structures(text);
    for node in &roots {
        descend_deserialize::<T>(text, node, &mut out);
    }
    out
}