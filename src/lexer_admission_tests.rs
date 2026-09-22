use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn template_scan_admits_exact_persistent_storage_and_boxing_overlap() {
    let frames = 4 * std::mem::size_of::<TemplateFrame>() as u64;
    let expressions = 4 * std::mem::size_of::<Span>() as u64;
    let boxed = std::mem::size_of::<Span>() as u64;
    let table = 4 * std::mem::size_of::<TemplateToken>() as u64;
    let mut owner = ledger(100_000, 1_000_000);
    let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
    let mut buffer = TokenBuffer::new("`${a}`", Some(&mut budget));
    assert_eq!(
        buffer.next().unwrap(),
        Some(Ok(TokenKind::TemplateLiteral(TemplateId(0))))
    );
    assert!(buffer.items.as_ref().unwrap().is_empty());
    let templates = &buffer.lexer.as_ref().unwrap().extras;
    assert_eq!(template_bytes(templates), table + boxed);
    assert_eq!(&*templates[0].expressions, &[Span::new(3, 4)]);
    buffer.budget.as_deref_mut().unwrap().with_ledger(|owner| {
        let (ledger, _) = owner.unwrap();
        assert_eq!(ledger.retained_bytes(), table + boxed);
        assert_eq!(
            ledger.peak_retained_bytes(),
            frames + expressions + boxed + table
        );
    });
    drop(buffer);
    drop(budget);
    assert_eq!(owner.retained_bytes(), 0);
}

#[test]
fn template_frame_expression_box_and_table_refusals_release_exact_owners() {
    let frame = std::mem::size_of::<TemplateFrame>() as u64;
    let span = std::mem::size_of::<Span>() as u64;
    let template = std::mem::size_of::<TemplateToken>() as u64;
    let cases = [
        // Fifth frame needs old capacity four plus new capacity eight.
        ("`a${`b${`c${value}`}`}`", 8 * frame, 4 * frame),
        // Fifth expression needs both old and new span vectors.
        (
            "`${a}${b}${c}${d}${e}`",
            4 * frame + 8 * span,
            4 * frame + 4 * span,
        ),
        // The exact boxed destination overlaps the growing expression vector.
        ("`${a}`", 4 * frame + 5 * span - 1, 4 * frame + 4 * span),
        // The completed boxed payload still belongs to scratch if table growth refuses.
        (
            "`${a}`",
            4 * frame + 5 * span + 4 * template - 1,
            4 * frame + 5 * span,
        ),
    ];
    for (source, memory, peak) in cases {
        let mut owner = ledger(100_000, memory + 19);
        owner.retain(WorkDomain::Baseline, 19).unwrap();
        let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
        assert_eq!(
            lex_admitted(source, &mut budget).unwrap_err(),
            AdmittedLexError::Resource(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(owner.retained_bytes(), 19);
        assert_eq!(owner.peak_retained_bytes(), peak + 19);
        owner.release(WorkDomain::Baseline, 19).unwrap();
    }
}

#[test]
fn template_work_refusals_and_malformed_tail_leave_no_retained_payload() {
    let source = "`first ${value} ${`nested ${other}`} final`";
    let mut calibrated = ledger(100_000, 1_000_000);
    let mut budget = AllocationBudget::new(Some((&mut calibrated, WorkDomain::Baseline)));
    let (tokens, bytes) = lex_admitted(source, &mut budget).unwrap();
    drop(tokens);
    budget.release(AllocationClass::Scratch, bytes).unwrap();
    drop(budget);
    assert_eq!(
        calibrated.work_by_kind(WorkKind::Analysis),
        (source.len() + source.len() - 1) as u64
    );
    let work = calibrated.work_used(WorkDomain::Baseline);
    for limit in 0..work {
        let mut owner = ledger(limit, 1_000_000);
        let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
        assert_eq!(
            lex_admitted(source, &mut budget).unwrap_err(),
            AdmittedLexError::Resource(AllocationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(owner.retained_bytes(), 0);
    }
    for source in ["`good ${a}` `missing", "`good ${a}` `bad ${/* missing"] {
        let mut owner = ledger(100_000, 1_000_000);
        let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
        assert_eq!(
            lex_admitted(source, &mut budget).unwrap_err(),
            AdmittedLexError::Syntax(lex(source).unwrap_err())
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(owner.retained_bytes(), 0);
    }
}

#[test]
fn template_scanner_deadline_and_unwind_preserve_preexisting_lexical_payload() {
    let mut owner = ledger(100_000, 1_000_000);
    let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
    let mut buffer = TokenBuffer::new("`old ${a}`", Some(&mut budget));
    buffer.next().unwrap().unwrap().unwrap();
    let retained = buffer
        .budget
        .as_deref()
        .unwrap()
        .retained_bytes(AllocationClass::Scratch);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let mut scratch = TemplateScratch {
            frames: Vec::new(),
            expressions: Vec::new(),
            boxed: None,
            budget: buffer.budget.as_deref_mut(),
        };
        scratch.frame(TemplateFrame::Text).unwrap();
        scratch.expression(Span::new(0, 1)).unwrap();
        let exact = scratch
            .budget
            .as_deref_mut()
            .unwrap()
            .copy_slice(AllocationClass::Scratch, &scratch.expressions)
            .unwrap();
        scratch.boxed = Some(exact.into_boxed_slice());
        panic!("injected template scanner panic");
    }));
    assert!(failure.is_err());
    assert_eq!(
        buffer
            .budget
            .as_deref()
            .unwrap()
            .retained_bytes(AllocationClass::Scratch),
        retained
    );
    assert_eq!(
        &*buffer.lexer.as_ref().unwrap().extras[0].expressions,
        &[Span::new(7, 8)]
    );
    {
        let mut scratch = TemplateScratch {
            frames: Vec::new(),
            expressions: Vec::new(),
            boxed: None,
            budget: buffer.budget.as_deref_mut(),
        };
        scratch.frame(TemplateFrame::Text).unwrap();
        scratch.budget.as_deref_mut().unwrap().with_ledger(|owner| {
            owner
                .unwrap()
                .0
                .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
        });
        let mut cursor = 1;
        assert_eq!(
            scratch.advance(&mut cursor, 1),
            Err(AllocationError::Budget(BudgetError::DeadlineExceeded))
        );
        assert_eq!(cursor, 1);
    }
    assert_eq!(
        buffer
            .budget
            .as_deref()
            .unwrap()
            .retained_bytes(AllocationClass::Scratch),
        retained
    );
    drop(buffer);
    drop(budget);
    assert_eq!(owner.retained_bytes(), 0);
}

#[test]
fn public_logos_and_admitted_driver_preserve_every_fixed_token() {
    let cases = [
        ("int", TokenKind::Int),
        ("float", TokenKind::Float),
        ("number", TokenKind::Number),
        ("string", TokenKind::String),
        ("bool", TokenKind::Bool),
        ("void", TokenKind::Void),
        ("auto", TokenKind::Auto),
        ("func", TokenKind::Func),
        ("struct", TokenKind::Struct),
        ("record", TokenKind::Record),
        ("enum", TokenKind::Enum),
        ("class", TokenKind::Class),
        ("extends", TokenKind::Extends),
        ("super", TokenKind::Super),
        ("return", TokenKind::Return),
        ("init", TokenKind::Init),
        ("if", TokenKind::If),
        ("else", TokenKind::Else),
        ("while", TokenKind::While),
        ("for", TokenKind::For),
        ("in", TokenKind::In),
        ("of", TokenKind::Of),
        ("break", TokenKind::Break),
        ("continue", TokenKind::Continue),
        ("extern", TokenKind::Extern),
        ("import", TokenKind::Import),
        ("export", TokenKind::Export),
        ("from", TokenKind::From),
        ("as", TokenKind::As),
        ("pure", TokenKind::Pure),
        ("true", TokenKind::True),
        ("false", TokenKind::False),
        ("null", TokenKind::Null),
        ("new", TokenKind::New),
        ("is", TokenKind::Is),
        ("match", TokenKind::Match),
        ("async", TokenKind::Async),
        ("generator", TokenKind::Generator),
        ("yield", TokenKind::Yield),
        ("await", TokenKind::Await),
        ("throw", TokenKind::Throw),
        ("try", TokenKind::Try),
        ("catch", TokenKind::Catch),
        ("finally", TokenKind::Finally),
        ("=>", TokenKind::FatArrow),
        ("...", TokenKind::Ellipsis),
        ("->", TokenKind::ThinArrow),
        ("==", TokenKind::EqEq),
        ("!=", TokenKind::BangEq),
        ("<=", TokenKind::LessEq),
        (">=", TokenKind::GreaterEq),
        ("&&", TokenKind::AndAnd),
        ("||", TokenKind::OrOr),
        ("??=", TokenKind::QuestionQuestionEq),
        ("??", TokenKind::QuestionQuestion),
        ("?.", TokenKind::QuestionDot),
        ("++", TokenKind::PlusPlus),
        ("--", TokenKind::MinusMinus),
        ("+=", TokenKind::PlusEq),
        ("-=", TokenKind::MinusEq),
        ("*=", TokenKind::StarEq),
        ("/=", TokenKind::SlashEq),
        ("%=", TokenKind::PercentEq),
        ("^=", TokenKind::CaretEq),
        ("&=", TokenKind::AmpersandEq),
        ("|=", TokenKind::PipeEq),
        ("<<=", TokenKind::ShiftLeftEq),
        (">>=", TokenKind::ShiftRightEq),
        (">>>=", TokenKind::UnsignedShiftRightEq),
        ("=", TokenKind::Eq),
        ("+", TokenKind::Plus),
        ("-", TokenKind::Minus),
        ("*", TokenKind::Star),
        ("/", TokenKind::Slash),
        ("%", TokenKind::Percent),
        ("^", TokenKind::Caret),
        ("&", TokenKind::Ampersand),
        ("<<", TokenKind::ShiftLeft),
        (">>", TokenKind::ShiftRight),
        (">>>", TokenKind::UnsignedShiftRight),
        ("!", TokenKind::Bang),
        ("<", TokenKind::Less),
        (">", TokenKind::Greater),
        (".", TokenKind::Dot),
        (",", TokenKind::Comma),
        (":", TokenKind::Colon),
        (";", TokenKind::Semicolon),
        ("(", TokenKind::LParen),
        (")", TokenKind::RParen),
        ("{", TokenKind::LBrace),
        ("}", TokenKind::RBrace),
        ("[", TokenKind::LBracket),
        ("]", TokenKind::RBracket),
        ("?", TokenKind::Question),
        ("@", TokenKind::At),
        ("|", TokenKind::Pipe),
        ("name_$", TokenKind::Ident("name_$")),
        ("123", TokenKind::IntLiteral(123)),
        ("1.25e2", TokenKind::FloatLiteral(125.0)),
        ("\"text\\n\"", TokenKind::StringLiteral("\"text\\n\"")),
    ];
    for (spelling, expected) in cases {
        let prefix = " /* skip */ ";
        let source = format!("{prefix}{spelling} // trailing");
        let span = Span::new(prefix.len(), prefix.len() + spelling.len());
        let mut public = TokenKind::lexer(&source);
        assert_eq!(TokenKind::lex(&mut public), Some(Ok(expected.clone())));
        assert_eq!(Span::from(public.span()), span);
        assert_eq!(public.slice(), spelling);
        assert_eq!(public.next(), None);
        let mut owner = ledger(100_000, 100_000);
        let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
        let (lexed, bytes) = lex_admitted(&source, &mut budget).unwrap();
        assert_eq!(
            lexed.items,
            [Token {
                kind: expected,
                span
            }]
        );
        drop(lexed);
        budget.release(AllocationClass::Scratch, bytes).unwrap();
        drop(budget);
        assert_eq!(owner.retained_bytes(), 0);
    }
}

#[test]
fn public_logos_preserves_template_extras_error_spans_and_direct_lex_state() {
    let old = lex("`old ${1}`").unwrap().templates;
    let source = " \n`fresh ${2}` + 9";
    let mut public = TokenKind::lexer_with_extras(source, old);
    assert_eq!(
        TokenKind::lex(&mut public),
        Some(Ok(TokenKind::TemplateLiteral(TemplateId(1))))
    );
    assert_eq!(
        Span::from(public.span()),
        Span::new(2, source.find(" +").unwrap())
    );
    assert_eq!(public.slice(), "`fresh ${2}`");
    assert_eq!(public.extras.len(), 2);
    assert_eq!(&*public.extras[1].expressions, &[Span::new(9, 10)]);
    assert_eq!(public.next(), Some(Ok(TokenKind::Plus)));
    assert_eq!(public.next(), Some(Ok(TokenKind::IntLiteral(9))));
    assert_eq!(public.next(), None);

    for source in ["`missing", "`missing ${\"quote}", "`missing ${/* comment}"] {
        let mut public = TokenKind::lexer(source);
        assert_eq!(public.next(), Some(Err(())));
        assert_eq!(public.span(), 0..1);
        assert!(public.extras.is_empty());
        assert_eq!(public.next(), Some(Ok(TokenKind::Ident("missing"))));
    }
    let mut public = TokenKind::lexer("a b");
    assert_eq!(TokenKind::lex(&mut public), Some(Ok(TokenKind::Ident("a"))));
    assert_eq!(TokenKind::lex(&mut public), Some(Ok(TokenKind::Ident("a"))));
    assert_eq!(public.span(), 0..1);
    assert_eq!(public.next(), Some(Ok(TokenKind::Ident("b"))));
}

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits {
            wall_time_ms: Some(100_000),
            ..ResourceLimits::default()
        },
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}

#[test]
fn ordinary_trivia_prepays_exact_input_bytes_without_allocating_tokens() {
    let sources = ["", " \t\n\r", "/*\u{03c0}\u{96ea}*/ //\u{00e9}\n"];
    assert!(sources[2].len() > sources[2].chars().count());
    for source in sources {
        let bytes = source.len() as u64;
        for work in [bytes.saturating_sub(1), bytes] {
            let mut owner = ledger(work, 19);
            owner.retain(WorkDomain::Baseline, 19).unwrap();
            let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
            let result = lex_admitted(source, &mut budget);
            if work < bytes {
                assert_eq!(
                    result.unwrap_err(),
                    AdmittedLexError::Resource(AllocationError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    ))
                );
            } else {
                let (tokens, retained) = result.unwrap();
                assert_eq!(tokens, lex(source).unwrap());
                assert!(tokens.is_empty());
                assert_eq!(retained, 0);
                drop(tokens);
            }
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            drop(budget);
            assert_eq!(
                owner.work_by_kind(WorkKind::Analysis),
                if work < bytes { 0 } else { bytes }
            );
            assert_eq!(owner.work_by_kind(WorkKind::Render), 0);
            assert_eq!(owner.retained_bytes(), 19);
            assert_eq!(owner.peak_retained_bytes(), 19);
        }
    }
}

#[test]
fn ordinary_literals_and_errors_require_byte_admission_before_token_storage() {
    let long = format!("\"{}\u{03c0}\"", "x".repeat(16_384));
    let overflow = "9".repeat(256);
    let sources = [long.as_str(), overflow.as_str(), "first #", "\u{03c0}"];
    for source in sources {
        let bytes = source.len() as u64;
        let mut refused = ledger(bytes - 1, 1_000_000);
        refused.retain(WorkDomain::Baseline, 19).unwrap();
        let mut budget = AllocationBudget::new(Some((&mut refused, WorkDomain::Baseline)));
        assert_eq!(
            lex_admitted(source, &mut budget).unwrap_err(),
            AdmittedLexError::Resource(AllocationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(refused.work_used(WorkDomain::Baseline), 0);
        assert_eq!(refused.peak_retained_bytes(), 19);
        assert_eq!(refused.retained_bytes(), 19);

        let expected = lex(source);
        let mut owner = ledger(100_000, 1_000_000);
        owner.retain(WorkDomain::Baseline, 19).unwrap();
        let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
        match (lex_admitted(source, &mut budget), expected) {
            (Ok((tokens, retained)), Ok(expected)) => {
                assert_eq!(tokens, expected);
                let TokenKind::StringLiteral(text) = tokens[0].kind else {
                    panic!("expected long borrowed string literal");
                };
                assert_eq!(text.as_ptr(), source.as_ptr());
                assert_eq!(text.len(), source.len());
                drop(tokens);
                budget.release(AllocationClass::Scratch, retained).unwrap();
            }
            (Err(AdmittedLexError::Syntax(actual)), Err(expected)) => {
                assert_eq!(actual, expected);
            }
            result => panic!("ordinary lexical behavior changed: {result:?}"),
        }
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(owner.work_by_kind(WorkKind::Analysis), bytes);
        assert_eq!(owner.retained_bytes(), 19);
    }
}

#[test]
fn completed_logos_calls_check_deadlines_for_tokens_errors_and_trivia_eof() {
    for source in ["held next", "held #", "held \t/* trailing */"] {
        let mut expected = TokenKind::lexer(source);
        assert_eq!(expected.next(), Some(Ok(TokenKind::Ident("held"))));
        let _ = expected.next();
        let expected_span = expected.span();
        let expected_remainder = expected.remainder();
        let mut owner = ledger(100_000, 1_000_000);
        owner.retain(WorkDomain::Baseline, 19).unwrap();
        let mut budget = AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
        let mut buffer = TokenBuffer::new(source, Some(&mut budget));
        let kind = buffer.next().unwrap().unwrap().unwrap();
        let span = Span::from(buffer.lexer.as_ref().unwrap().span());
        buffer.push(Token { kind, span }).unwrap();
        let original = buffer.items.as_ref().unwrap().as_ptr();
        let retained = token_bytes(buffer.items.as_ref().unwrap());
        let before_span = buffer.lexer.as_ref().unwrap().span();
        buffer.budget.as_deref_mut().unwrap().with_ledger(|owner| {
            owner
                .unwrap()
                .0
                .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
        });
        assert_eq!(
            buffer.next(),
            Err(AdmittedLexError::Resource(AllocationError::Budget(
                BudgetError::DeadlineExceeded
            )))
        );
        let raw = buffer.lexer.as_ref().unwrap();
        assert_eq!(raw.span(), expected_span);
        assert_eq!(raw.remainder(), expected_remainder);
        assert_ne!(
            raw.span(),
            before_span,
            "the Logos call completed before its deadline check"
        );
        let items = buffer.items.as_ref().unwrap();
        assert_eq!(items.as_ptr(), original);
        assert_eq!(
            items,
            &[Token {
                kind: TokenKind::Ident("held"),
                span: Span::new(0, 4)
            }]
        );
        buffer.budget.as_deref_mut().unwrap().with_ledger(|owner| {
            assert_eq!(owner.unwrap().0.retained_bytes(), 19 + retained);
        });
        drop(buffer);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(owner.retained_bytes(), 19);
    }
}

fn token(index: usize) -> Token<'static> {
    Token {
        kind: TokenKind::Ident("value"),
        span: Span::new(index, index + 1),
    }
}

#[test]
fn admitted_token_streams_match_public_lexer_without_rebinding_source() {
    for source in [
        "",
        "/* comment */ int value=17; // line\n value+=2;",
        "string text=\"escaped\\n\\uD800\";float number=1.25;",
        "string text=`outer ${`inner ${value}`} ${\"}\" /* } */}`;",
    ] {
        let expected = lex(source).unwrap();
        let mut ledger = ledger(100_000, 1_000_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let (actual, bytes) = lex_admitted(source, &mut budget).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            bytes,
            token_bytes(&actual.items) + template_bytes(&actual.templates)
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), bytes);
        for value in &actual {
            if let TokenKind::Ident(text) | TokenKind::StringLiteral(text) = value.kind {
                assert_eq!(text.as_ptr(), source[value.span.start..].as_ptr());
            }
        }
        drop(actual);
        budget.release(AllocationClass::Scratch, bytes).unwrap();
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn token_refusal_precedes_first_vector_allocation() {
    for (work, memory, expected) in [
        (100, 0, BudgetError::MemoryExhausted(WorkDomain::Baseline)),
        (0, 100_000, BudgetError::WorkExhausted(WorkDomain::Baseline)),
    ] {
        let mut ledger = ledger(work, memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        assert_eq!(
            lex_admitted("value", &mut budget).unwrap_err(),
            AdmittedLexError::Resource(AllocationError::Budget(expected))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.peak_retained_bytes(), 0);
    }
}

#[test]
fn failed_growth_preserves_tokens_and_requires_old_plus_new_capacity() {
    let element = std::mem::size_of::<Token<'static>>() as u64;
    let mut ledger = ledger(100_000, 8 * element);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let mut buffer = TokenBuffer::new("", Some(&mut budget));
    for index in 0..4 {
        buffer.push(token(index)).unwrap();
    }
    let original = buffer.items.as_ref().unwrap().as_ptr();
    assert_eq!(
        buffer.push(token(4)),
        Err(AdmittedLexError::Resource(AllocationError::Budget(
            BudgetError::MemoryExhausted(WorkDomain::Baseline)
        )))
    );
    let items = buffer.items.as_ref().unwrap();
    assert_eq!(items.as_ptr(), original);
    assert_eq!(items.capacity(), 4);
    assert_eq!(items, &(0..4).map(token).collect::<Vec<_>>());
    assert_eq!(
        buffer
            .budget
            .as_ref()
            .unwrap()
            .retained_bytes(AllocationClass::Scratch),
        4 * element
    );
    drop(buffer);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.peak_retained_bytes(), 4 * element);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn successful_growth_records_live_capacity_and_overlap_peak() {
    let element = std::mem::size_of::<Token<'static>>() as u64;
    let mut ledger = ledger(100_000, 12 * element);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let (tokens, bytes) = lex_admitted("a b c d e", &mut budget).unwrap();
    assert_eq!(tokens.len(), 5);
    assert_eq!(bytes, 8 * element);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), bytes);
    budget.with_ledger(|owner| {
        let (ledger, _) = owner.unwrap();
        assert_eq!(ledger.peak_retained_bytes(), 12 * element);
    });
    drop(tokens);
    budget.release(AllocationClass::Scratch, bytes).unwrap();
    drop(budget);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn lexical_syntax_failure_and_panic_release_only_owned_tokens() {
    let mut ledger = ledger(100_000, 1_000_000);
    ledger.retain(WorkDomain::Baseline, 19).unwrap();
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let source = "a b c d e #";
    assert_eq!(
        lex_admitted(source, &mut budget).unwrap_err(),
        AdmittedLexError::Syntax(lex(source).unwrap_err())
    );
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let mut buffer = TokenBuffer::new("", Some(&mut budget));
        buffer.push(token(0)).unwrap();
        panic!("injected token production panic");
    }));
    assert!(failure.is_err());
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), 19);
    ledger.release(WorkDomain::Baseline, 19).unwrap();
}

#[test]
fn expired_lexing_refuses_before_scanning_and_still_releases_token_storage() {
    let mut ledger = ledger(100_000, 1_000_000);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let mut buffer = TokenBuffer::new("", Some(&mut budget));
    buffer.push(token(0)).unwrap();
    buffer.budget.as_mut().unwrap().with_ledger(|owner| {
        owner
            .unwrap()
            .0
            .set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
    });
    assert_eq!(
        buffer.push(token(1)),
        Err(AdmittedLexError::Resource(AllocationError::Budget(
            BudgetError::DeadlineExceeded
        )))
    );
    assert_eq!(buffer.items.as_ref().unwrap().len(), 1);
    drop(buffer);
    assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    for source in ["", " /* skipped */ ", "#"] {
        assert_eq!(
            lex_admitted(source, &mut budget).unwrap_err(),
            AdmittedLexError::Resource(AllocationError::Budget(BudgetError::DeadlineExceeded))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
    }
    drop(budget);
    assert_eq!(ledger.retained_bytes(), 0);
}
