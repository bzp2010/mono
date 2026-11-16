use mono_release::{parse_record, Bump};

#[test]
fn a_record_lists_bumps_per_project_and_keeps_its_summary() {
    let text = "---\n\"my-lib\": minor\n\"my-app\": patch\n---\n\nAdded a parser option.\n";
    let record = parse_record(text).unwrap();
    assert_eq!(record.bumps.get("my-lib"), Some(&Bump::Minor));
    assert_eq!(record.bumps.get("my-app"), Some(&Bump::Patch));
    assert_eq!(record.summary.trim(), "Added a parser option.");
}

#[test]
fn an_unknown_bump_keyword_is_an_error() {
    assert!(parse_record("---\n\"my-lib\": huge\n---\n\nx\n").is_err());
}

#[test]
fn a_record_without_frontmatter_is_an_error() {
    assert!(parse_record("just a summary\n").is_err());
}

#[test]
fn crlf_line_endings_are_accepted() {
    let record = parse_record("---\r\n\"my-lib\": patch\r\n---\r\n\r\nFixed it.\r\n").unwrap();
    assert_eq!(record.bumps.get("my-lib"), Some(&Bump::Patch));
    assert_eq!(record.summary.trim(), "Fixed it.");
}

#[test]
fn a_closing_delimiter_must_be_a_whole_line() {
    assert!(parse_record("---\n\"my-lib\": patch\n---oops\nbody\n").is_err());
}

#[test]
fn a_record_may_end_at_the_closing_delimiter() {
    let record = parse_record("---\n\"my-lib\": patch\n---").unwrap();
    assert_eq!(record.summary, "");
}
