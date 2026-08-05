#![allow(clippy::expect_used, clippy::unwrap_used)]

use event_sourcing::*;

#[test]
fn test_read_options_builder() {
    let opts = ReadOptions::new()
        .after(SequencePosition::new(42))
        .limit(100)
        .direction(Direction::Backward);

    assert_eq!(opts.after, Some(SequencePosition::new(42)));
    assert_eq!(opts.limit, Some(100));
    assert_eq!(opts.direction, Direction::Backward);
}
