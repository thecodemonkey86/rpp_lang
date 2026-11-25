use std::io::{self, BufRead, Cursor, Write};

pub(crate) enum Console {
    Captured {
        input: Cursor<String>,
        stdout: String,
        stderr: String,
    },
    Stdio,
}

impl Console {
    pub(crate) fn captured(input: String) -> Self {
        Self::Captured {
            input: Cursor::new(input),
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    pub(crate) fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        let read = match self {
            Self::Captured { input, .. } => input.read_line(&mut line),
            Self::Stdio => io::stdin().read_line(&mut line),
        };
        if read.ok()? == 0 {
            return None;
        }
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        Some(line)
    }

    pub(crate) fn write_stdout(&mut self, text: &str) {
        match self {
            Self::Captured { stdout, .. } => stdout.push_str(text),
            Self::Stdio => {
                // Show prompts even if they do not end in a newline.
                let mut stdout = io::stdout().lock();
                stdout.write_all(text.as_bytes())
                    .and_then(|()| stdout.flush())
                    .expect("could not write to stdout");
            }
        }
    }

    pub(crate) fn write_stderr(&mut self, text: &str) {
        match self {
            Self::Captured { stderr, .. } => stderr.push_str(text),
            Self::Stdio => {
                io::stderr().lock().write_all(text.as_bytes())
                    .expect("could not write to stderr");
            }
        }
    }

    pub(crate) fn into_output(self) -> (String, String) {
        match self {
            Self::Captured { stdout, stderr, .. } => (stdout, stderr),
            Self::Stdio => (String::new(), String::new()),
        }
    }
}
