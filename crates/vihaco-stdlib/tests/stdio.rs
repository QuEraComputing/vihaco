// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::fs::{self, File, OpenOptions};
use std::process::Command;

use eyre::Result;
use vihaco_runtime::Observe;
use vihaco_stdlib::observer::stdio::{StdoutEffect, StdoutObserver};

fn emit_output(observer: &mut StdoutObserver) -> Result<()> {
    observer.write_stdout("first: ")?;
    for text in ["α", "", "\nlast: 🐦"] {
        let follow_ups = observer.observe(&StdoutEffect(text.into()))?;
        assert!(follow_ups.into_iter().next().is_none());
    }
    observer.flush()
}

#[test]
fn buffers_preserve_text_and_remain_independent() -> Result<()> {
    let mut default = StdoutObserver::default();
    let mut explicit = StdoutObserver::buffered();
    assert!(default.output().is_empty());
    assert!(explicit.output().is_empty());

    emit_output(&mut default)?;
    assert!(explicit.output().is_empty());
    emit_output(&mut explicit)?;
    assert_eq!(default.output(), "first: α\nlast: 🐦".as_bytes());
    assert_eq!(default.output(), explicit.output());

    explicit.write_stdout("!")?;
    assert_eq!(default.output(), "first: α\nlast: 🐦".as_bytes());
    assert_eq!(explicit.output(), "first: α\nlast: 🐦!".as_bytes());
    Ok(())
}

#[test]
fn file_output_matches_buffer_output() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stdout.txt");
    let mut file = StdoutObserver::file(File::create(&path)?);
    let mut buffer = StdoutObserver::buffered();

    emit_output(&mut file)?;
    emit_output(&mut buffer)?;
    assert_eq!(fs::read(path)?, buffer.output());
    assert!(file.output().is_empty());
    Ok(())
}

#[test]
fn caller_can_append_to_an_existing_file() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stdout.txt");
    fs::write(&path, "previous output\n")?;
    let file = OpenOptions::new().append(true).open(&path)?;
    let mut observer = StdoutObserver::file(file);

    emit_output(&mut observer)?;
    assert_eq!(
        fs::read_to_string(path)?,
        "previous output\nfirst: α\nlast: 🐦"
    );
    Ok(())
}

#[test]
fn read_only_files_propagate_write_errors() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stdout.txt");
    fs::write(&path, "original")?;
    let mut observer = StdoutObserver::file(File::open(&path)?);

    let error = observer.write_stdout("direct write").unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    let error = observer
        .observe(&StdoutEffect("observed write".into()))
        .unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(fs::read_to_string(path)?, "original");
    Ok(())
}

#[test]
fn only_stdout_destination_writes_to_process_stdout() -> Result<()> {
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", "stdout_routing_child", "--nocapture"])
        .env("VIHACO_STDOUT_ROUTING_CHILD", "1")
        .output()?;
    assert!(output.status.success(), "child failed: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("terminal marker: α"), "{stdout}");
    assert!(!stdout.contains("buffer marker: β"), "{stdout}");
    assert!(!stdout.contains("file marker: γ"), "{stdout}");
    Ok(())
}

// A subprocess gives this test its own process stdout without changing global state.
#[test]
fn stdout_routing_child() -> Result<()> {
    if std::env::var_os("VIHACO_STDOUT_ROUTING_CHILD").is_none() {
        return Ok(());
    }

    let mut stdout = StdoutObserver::stdout();
    stdout.observe(&StdoutEffect("terminal marker: α".into()))?;
    stdout.flush()?;
    assert!(stdout.output().is_empty());

    for mut buffer in [StdoutObserver::default(), StdoutObserver::buffered()] {
        buffer.observe(&StdoutEffect("buffer marker: β".into()))?;
        buffer.flush()?;
        assert_eq!(buffer.output(), "buffer marker: β".as_bytes());
    }

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stdout.txt");
    let mut file = StdoutObserver::file(File::create(&path)?);
    file.observe(&StdoutEffect("file marker: γ".into()))?;
    file.flush()?;
    assert_eq!(fs::read_to_string(path)?, "file marker: γ");
    assert!(file.output().is_empty());
    Ok(())
}
