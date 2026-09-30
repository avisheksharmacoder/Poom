use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffInlineChunk {
    pub tag: ChangeType,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    pub tag: ChangeType,
    pub old_index: Option<usize>,
    pub new_index: Option<usize>,
    pub content: String,
    pub inline_chunks: Vec<DiffInlineChunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult {
    pub lines: Vec<DiffLine>,
    pub additions: usize,
    pub deletions: usize,
    pub unchanged: usize,
    pub similarity_ratio: f64,
}

impl DiffResult {
    /// Computes two-tier diff (line-level + inline word-level) between two text strings.
    pub fn compute(old_text: &str, new_text: &str) -> Self {
        let diff = TextDiff::from_lines(old_text, new_text);

        let mut lines = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;
        let mut unchanged = 0;

        for change in diff.iter_all_changes() {
            let tag = match change.tag() {
                ChangeTag::Equal => {
                    unchanged += 1;
                    ChangeType::Equal
                }
                ChangeTag::Insert => {
                    additions += 1;
                    ChangeType::Insert
                }
                ChangeTag::Delete => {
                    deletions += 1;
                    ChangeType::Delete
                }
            };

            let content = change.value().trim_end_matches(['\r', '\n']).to_string();

            lines.push(DiffLine {
                tag,
                old_index: change.old_index(),
                new_index: change.new_index(),
                content,
                inline_chunks: Vec::new(),
            });
        }

        // Tier 2: Inline word diff for modified adjacent blocks (Delete followed by Insert)
        let mut i = 0;
        while i < lines.len() {
            if lines[i].tag == ChangeType::Delete
                && i + 1 < lines.len()
                && lines[i + 1].tag == ChangeType::Insert
            {
                let old_line_str = &lines[i].content;
                let new_line_str = &lines[i + 1].content;

                let word_diff = TextDiff::from_words(old_line_str, new_line_str);

                let mut del_chunks = Vec::new();
                let mut ins_chunks = Vec::new();

                for w_change in word_diff.iter_all_changes() {
                    match w_change.tag() {
                        ChangeTag::Equal => {
                            del_chunks.push(DiffInlineChunk {
                                tag: ChangeType::Equal,
                                text: w_change.value().to_string(),
                            });
                            ins_chunks.push(DiffInlineChunk {
                                tag: ChangeType::Equal,
                                text: w_change.value().to_string(),
                            });
                        }
                        ChangeTag::Delete => {
                            del_chunks.push(DiffInlineChunk {
                                tag: ChangeType::Delete,
                                text: w_change.value().to_string(),
                            });
                        }
                        ChangeTag::Insert => {
                            ins_chunks.push(DiffInlineChunk {
                                tag: ChangeType::Insert,
                                text: w_change.value().to_string(),
                            });
                        }
                    }
                }

                lines[i].inline_chunks = del_chunks;
                lines[i + 1].inline_chunks = ins_chunks;
                i += 2;
            } else {
                i += 1;
            }
        }

        let similarity_ratio = TextDiff::from_chars(old_text, new_text).ratio() as f64;

        Self {
            lines,
            additions,
            deletions,
            unchanged,
            similarity_ratio,
        }
    }
}
