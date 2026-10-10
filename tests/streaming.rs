//! The writer streams its file to an `io::Write`: text reaches the output
//! before `finish`, a failing output is reported by `finish` and nowhere
//! else, and a rejected `new` writes nothing.

mod common;

use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use brep_to_step::{Error, Header, StepWriter, Units};
use common::fixtures::{LineKind, cube};
use common::scene::{Scene, header, replay, write_ours};

/// An output the test can read while the writer holds it.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An output with room for `room` bytes, failing once it is full.
struct Full {
    room: usize,
}

impl Write for Full {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.room == 0 {
            return Err(io::Error::other("the disk is full"));
        }
        let taken = buf.len().min(self.room);
        self.room -= taken;
        Ok(taken)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `count` unit cubes in a row, one part each — about 8 KB of text a cube.
fn cubes(count: usize) -> Scene {
    let mut scene = cube("cube 0", [0.0; 3], 1.0, LineKind::Along);
    for i in 1..count {
        let x = f64::from(u32::try_from(i).expect("few cubes")) * 2.0;
        scene.merge(cube(
            &format!("cube {i}"),
            [x, 0.0, 0.0],
            1.0,
            LineKind::Along,
        ));
    }
    scene
}

#[test]
fn streams_before_finish() {
    let scene = cubes(20);
    let out = Shared::default();
    let mut w = StepWriter::new(out.clone(), &header(), Units::default()).expect("writer");
    assert!(
        out.0.borrow().is_empty(),
        "the header and skeleton are less than a chunk, so they wait"
    );

    replay(&mut w, &scene);
    let sent = out.0.borrow().len();
    assert!(sent >= 64 * 1024, "only {sent} bytes sent before finish");

    w.finish().expect("finish");
    let file = String::from_utf8(out.0.borrow().clone()).expect("ASCII");
    assert_eq!(
        file,
        write_ours(&scene),
        "the same file, whatever the output"
    );
}

#[test]
fn io_error_is_reported_at_finish() {
    let scene = cubes(20);
    // Full at once, during the first chunk, and during a later one.
    for room in [0, 1_000, 100_000] {
        let mut w = StepWriter::new(Full { room }, &header(), Units::default()).expect("writer");
        // `replay` expects every call to succeed: a failing output does not
        // make the calls fail.
        replay(&mut w, &scene);
        let result = w.finish();
        let Err(Error::Io(e)) = &result else {
            panic!(
                "room {room}: expected Error::Io, got {:?}",
                result.map(|_| ())
            );
        };
        assert_eq!(e.to_string(), "the disk is full");
        let error = result.err().expect("an error");
        assert!(std::error::Error::source(&error).is_some());
        assert!(error.to_string().starts_with("writing the file failed"));
    }
}

#[test]
fn rejected_new_writes_nothing() {
    let mut out = Vec::new();
    let long = Header {
        file_name: "a".repeat(300),
        ..header()
    };
    assert!(matches!(
        StepWriter::new(&mut out, &long, Units::default()),
        Err(Error::HeaderTooLong { .. })
    ));
    let bad = Units {
        uncertainty: f64::NAN,
        ..Units::default()
    };
    assert!(matches!(
        StepWriter::new(&mut out, &header(), bad),
        Err(Error::InvalidNumber { .. })
    ));
    assert_eq!(out, Vec::<u8>::new(), "nothing written");
}
