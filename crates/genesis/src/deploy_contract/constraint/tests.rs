use super::*;
use crate::utils::{constraint::*, test_access, TestAccess};

#[test]
fn test_push_predicate_i() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup_default();
    repeat.repeat_to(0, 50).unwrap();
    repeat.repeat().unwrap();
    let ops = push_predicate_i();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![3]);
}

#[test]
fn test_read_predicate() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, _) = setup_default();
    let access = test_access!({
        let mut access = TestAccess::default().with_default_sol_data();
        access.data[0].decision_variables = vec![vec![0], vec![0], vec![0], vec![0, 24, 2, 3, 4]];
        access
    });
    repeat.repeat_to(0, 50).unwrap();
    repeat.repeat().unwrap();
    let ops = read_predicate_len();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![5]);
    stack.pop().unwrap();

    let ops = read_predicate_words_len();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![3]);
    stack.pop().unwrap();

    let ops = read_predicate_bytes_len();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![24]);
    stack.pop().unwrap();

    let ops = read_predicate_tag();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![0]);
    stack.pop().unwrap();

    let ops = read_predicate_words();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4]);
    stack.pop().unwrap();
    stack.pop().unwrap();
    stack.pop().unwrap();

    let ops = read_predicate_word_i(1);
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();
    assert_eq!(stack.iter().copied().collect::<Vec<_>>(), vec![3]);
    stack.pop().unwrap();
}
