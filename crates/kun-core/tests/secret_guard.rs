//! Tests for the Secret Guard, through its public interface only.
//!
//! Every fixture that looks like a secret is built by concatenating parts
//! (and usually contains `EXAMPLE` or `FAKE`), so no real-looking credential
//! sits in the repository as a single literal.

use kun_core::secret_guard::{
    Decision, Finding, GuardPolicy, SecretGuard, SecretGuardError, SecretKind, decide, redact,
};

/// A Secret Guard with only the built-in patterns.
fn guard() -> SecretGuard {
    SecretGuard::new(&[]).expect("built-in patterns compile")
}

/// The kinds found in `text`, in order of appearance.
fn kinds_in(text: &str) -> Vec<SecretKind> {
    guard().scan(text).iter().map(|f| f.kind).collect()
}

// ---------------------------------------------------------------------------
// must-detect
// ---------------------------------------------------------------------------

#[test]
fn detects_aws_access_key_id() {
    let key = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let text = format!("aws_access_key_id = {key}");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::AwsAccessKey);
    assert_eq!(&text[findings[0].range.clone()], key);
}

#[test]
fn detects_aws_temporary_access_key_id() {
    let key = format!("{}{}", "ASIA", "FAKEFAKEEXAMPLE1");
    assert_eq!(kinds_in(&key), vec![SecretKind::AwsAccessKey]);
}

#[test]
fn detects_github_tokens_with_every_prefix() {
    // 36 characters after the prefix, like a real classic token.
    let body = format!("{}{}", "FAKEexample0123456789", "abcdefghijklmno");
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        let token = format!("{prefix}{body}");
        let text = format!("token: {token}\n");
        assert_eq!(
            kinds_in(&text),
            vec![SecretKind::GitHubToken],
            "prefix {prefix}"
        );
    }

    let fine_grained = format!("{}{}{}", "github_pat_", "11FAKEEXAMPLE0123456789_", body);
    assert_eq!(kinds_in(&fine_grained), vec![SecretKind::GitHubToken]);
}

#[test]
fn detects_anthropic_api_key_once_as_anthropic() {
    let key = format!(
        "{}{}{}",
        "sk-ant-", "api03-", "FAKE_EXAMPLE-0123456789abcdefghijklmnopqrstuvwxyz"
    );
    let text = format!("ANTHROPIC_API_KEY={key}");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::AnthropicApiKey);
    assert_eq!(&text[findings[0].range.clone()], key);
}

#[test]
fn detects_openai_legacy_and_project_keys() {
    let legacy = format!(
        "{}{}",
        "sk-", "FAKEEXAMPLE0123456789abcdefghijklmnopqrstuvwxyzAB"
    );
    assert_eq!(kinds_in(&legacy), vec![SecretKind::OpenAiApiKey]);

    let project = format!(
        "{}{}",
        "sk-proj-", "FAKE_EXAMPLE-0123456789abcdefghijklmnopqrstuvwxyz"
    );
    assert_eq!(kinds_in(&project), vec![SecretKind::OpenAiApiKey]);
}

#[test]
fn detects_google_api_key() {
    // `AIza` + exactly 35 characters.
    let key = format!("{}{}", "AIza", "SyFAKE-EXAMPLE_0123456789abcdefghij");
    assert_eq!(key.len(), 4 + 35);
    let text = format!("key={key}&q=hello");
    assert_eq!(kinds_in(&text), vec![SecretKind::GoogleApiKey]);
}

#[test]
fn detects_slack_tokens() {
    for prefix in ["xoxa-", "xoxb-", "xoxp-", "xoxr-", "xoxs-"] {
        let token = format!("{prefix}{}", "0000000000-FAKE-EXAMPLE");
        assert_eq!(
            kinds_in(&token),
            vec![SecretKind::SlackToken],
            "prefix {prefix}"
        );
    }
}

/// A fake PEM block: `-----BEGIN <label>PRIVATE KEY-----` … `-----END …`.
fn fake_pem(label: &str) -> String {
    let dashes = "-----";
    format!(
        "{dashes}BEGIN {label}PRIVATE KEY{dashes}\nFAKEEXAMPLEFAKEEXAMPLE\nNOTAREALKEY==\n{dashes}END {label}PRIVATE KEY{dashes}"
    )
}

#[test]
fn detects_whole_pem_private_key_block() {
    for label in ["", "RSA ", "EC ", "OPENSSH ", "ENCRYPTED "] {
        let block = fake_pem(label);
        let text = format!("before\n{block}\nafter");

        let findings = guard().scan(&text);

        assert_eq!(findings.len(), 1, "label {label:?}");
        assert_eq!(findings[0].kind, SecretKind::PrivateKeyBlock);
        assert_eq!(&text[findings[0].range.clone()], block);
    }
}

#[test]
fn truncated_pem_block_ends_after_its_last_base64_line() {
    let block = format!(
        "{}{}{}",
        "-----BEGIN RSA PRIVATE KEY-----\n",
        "FAKEEXAMPLEFAKEEXAMPLE0123456789\n",
        "NOTAREALKEYNOTAREALKEY+/=="
    );
    let text = format!("{block}\nThe rest of this message is not a key.");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::PrivateKeyBlock);
    assert_eq!(&text[findings[0].range.clone()], block);
    assert_eq!(
        redact(&text, &findings),
        "[REDACTED]\nThe rest of this message is not a key."
    );
}

#[test]
fn detects_encrypted_legacy_pem_block_with_header_lines() {
    let dashes = "-----";
    let block = format!(
        "{dashes}BEGIN RSA PRIVATE KEY{dashes}\nProc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,FAKE0123\n\nFAKEEXAMPLEFAKEEXAMPLE0123456789\nNOTAREALKEY==\n{dashes}END RSA PRIVATE KEY{dashes}"
    );
    let text = format!("key:\n{block}\ndone");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(&text[findings[0].range.clone()], block);
}

#[test]
fn detects_pem_block_escaped_inside_json() {
    // Service-account JSON files write newlines as the two characters `\n`.
    let block = format!(
        "{}{}{}",
        r"-----BEGIN PRIVATE KEY-----\n",
        r"FAKEEXAMPLEFAKEEXAMPLE0123456789\nNOTAREALKEY==\n",
        "-----END PRIVATE KEY-----"
    );
    let text = format!(r#"{{"private_key": "{block}\n", "type": "FAKE"}}"#);

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::PrivateKeyBlock);
    assert_eq!(&text[findings[0].range.clone()], block);
}

#[test]
fn detects_jwt() {
    // header `{"alg":"HS256"}`, a fake payload, a fake signature.
    let jwt = format!(
        "{}.{}.{}",
        "eyJhbGciOiJIUzI1NiJ9", "eyJzdWIiOiJGQUtFLUVYQU1QTEUifQ", "FAKE_EXAMPLE-signature"
    );
    let text = format!("Authorization: Bearer {jwt}");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::Jwt);
    assert_eq!(&text[findings[0].range.clone()], jwt);
}

#[test]
fn detects_signed_jwt_with_empty_claims() {
    // Payload `e30` is base64url for `{}`: only 3 characters.
    let jwt = format!(
        "{}.{}.{}",
        "eyJhbGciOiJIUzI1NiJ9", "e30", "FAKE_EXAMPLE-signature"
    );
    let text = format!("token {jwt} end");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, SecretKind::Jwt);
    assert_eq!(&text[findings[0].range.clone()], jwt);
}

/// One fake sample of every built-in kind.
fn one_sample_per_kind() -> Vec<(SecretKind, String)> {
    vec![
        (
            SecretKind::AwsAccessKey,
            format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE"),
        ),
        (
            SecretKind::GitHubToken,
            format!("{}{}", "ghp_", "FAKEexample0123456789abcdefghijklmno"),
        ),
        (
            SecretKind::AnthropicApiKey,
            format!("{}{}", "sk-ant-api03-", "FAKE_EXAMPLE-0123456789abcdefghij"),
        ),
        (
            SecretKind::OpenAiApiKey,
            format!(
                "{}{}",
                "sk-", "FAKEEXAMPLE0123456789abcdefghijklmnopqrstuvwxyzAB"
            ),
        ),
        (
            SecretKind::GoogleApiKey,
            format!("{}{}", "AIza", "SyFAKE-EXAMPLE_0123456789abcdefghij"),
        ),
        (
            SecretKind::SlackToken,
            format!("{}{}", "xoxb-", "0000000000-FAKE-EXAMPLE"),
        ),
        (SecretKind::PrivateKeyBlock, fake_pem("RSA ")),
        (
            SecretKind::Jwt,
            format!(
                "{}.{}.{}",
                "eyJhbGciOiJIUzI1NiJ9", "e30", "FAKE_EXAMPLE-signature"
            ),
        ),
    ]
}

#[test]
fn detects_every_kind_glued_to_thai_text() {
    // Thai is written without spaces, so a key often touches Thai letters.
    for (kind, secret) in one_sample_per_kind() {
        let text = format!("กุญแจคือ{secret}ของฉัน");

        let findings = guard().scan(&text);

        assert_eq!(findings.len(), 1, "{kind:?} not found once in Thai text");
        assert_eq!(findings[0].kind, kind);
        assert_eq!(&text[findings[0].range.clone()], secret, "{kind:?}");
    }
}

#[test]
fn redacts_a_key_glued_to_thai_text_and_keeps_the_thai_intact() {
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let text = format!("กุญแจคือ{aws}ของฉัน");

    let redacted = redact(&text, &guard().scan(&text));

    assert_eq!(redacted, "กุญแจคือ[REDACTED]ของฉัน");
    assert!(!redacted.contains("IOSFODNN7"));
}

#[test]
fn findings_are_in_order_of_appearance() {
    let slack = format!("{}{}", "xoxb-", "0000000000-FAKE-EXAMPLE");
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let text = format!("first {slack} then {aws}");

    assert_eq!(
        kinds_in(&text),
        vec![SecretKind::SlackToken, SecretKind::AwsAccessKey]
    );
}

#[test]
fn finding_debug_and_display_never_contain_the_secret() {
    let key = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let text = format!("key {key}");
    let findings = guard().scan(&text);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];

    let debug = format!("{finding:?}");
    let pretty_debug = format!("{finding:#?}");
    let display = finding.to_string();

    for shown in [&debug, &pretty_debug, &display] {
        assert!(!shown.contains(&key), "secret leaked: {shown}");
        assert!(
            !shown.contains("IOSFODNN7"),
            "part of secret leaked: {shown}"
        );
    }
    // Display is still useful: it says what was found and where.
    assert_eq!(display, "possible AWS access key at bytes 4..24");
}

// ---------------------------------------------------------------------------
// extra patterns
// ---------------------------------------------------------------------------

#[test]
fn extra_pattern_is_reported_as_custom_alongside_built_ins() {
    let guard = SecretGuard::new(&[r"ACME-[0-9]{6}".to_string()]).expect("valid pattern");
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let text = format!("internal ACME-123456 and {aws}");

    let kinds: Vec<SecretKind> = guard.scan(&text).iter().map(|f| f.kind).collect();

    assert_eq!(kinds, vec![SecretKind::Custom, SecretKind::AwsAccessKey]);
}

#[test]
fn invalid_extra_pattern_is_an_error_not_a_panic() {
    let extra = vec!["ACME-[0-9]{6}".to_string(), "unclosed(".to_string()];

    let result = SecretGuard::new(&extra);

    match result {
        Err(SecretGuardError::InvalidPattern { index, .. }) => assert_eq!(index, 1),
        Ok(_) => panic!("expected an error for an invalid pattern"),
    }
}

#[test]
fn invalid_pattern_error_never_shows_the_pattern() {
    // A user may paste a real key as a pattern; an invalid one must not leak.
    let leaky = format!("{}{}(", "AKIA", "IOSFODNN7EXAMPLE");
    let error = match SecretGuard::new(&[leaky]) {
        Err(error) => error,
        Ok(_) => panic!("expected an error for an invalid pattern"),
    };

    let display = error.to_string();
    let debug = format!("{error:?}");
    let pretty_debug = format!("{error:#?}");
    for shown in [&display, &debug, &pretty_debug] {
        assert!(!shown.contains("IOSFODNN7"), "pattern leaked: {shown}");
    }
    assert!(
        std::error::Error::source(&error).is_none(),
        "the regex error (which quotes the pattern) must not be attached"
    );
    assert_eq!(display, "extra secret pattern #0 is not a valid regex");
}

#[test]
fn extra_pattern_matching_empty_text_finds_nothing_in_clean_text() {
    let guard = SecretGuard::new(&["x*".to_string()]).expect("valid pattern");

    assert!(guard.scan("hello world").is_empty());
}

// ---------------------------------------------------------------------------
// redact
// ---------------------------------------------------------------------------

#[test]
fn redact_replaces_each_finding_and_keeps_the_rest() {
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let slack = format!("{}{}", "xoxb-", "0000000000-FAKE-EXAMPLE");
    let text = format!("a {aws} b {slack} c");
    let findings = guard().scan(&text);

    assert_eq!(redact(&text, &findings), "a [REDACTED] b [REDACTED] c");
}

fn custom(range: std::ops::Range<usize>) -> Finding {
    Finding {
        kind: SecretKind::Custom,
        range,
    }
}

#[test]
fn redact_merges_overlapping_findings_from_scan() {
    // The extra pattern overlaps the end of the AWS key and runs past it.
    let guard = SecretGuard::new(&["EXAMPLE-[0-9]+".to_string()]).expect("valid pattern");
    let text = format!("x {}{} y", "AKIA", "IOSFODNN7EXAMPLE-42");
    let findings = guard.scan(&text);
    assert_eq!(findings.len(), 2);

    assert_eq!(redact(&text, &findings), "x [REDACTED] y");
}

#[test]
fn redact_handles_nested_adjacent_and_unsorted_findings() {
    //          0123456789
    let text = "abcdefghij";

    // Nested: 3..5 lies inside 2..7.
    assert_eq!(
        redact(text, &[custom(2..7), custom(3..5)]),
        "ab[REDACTED]hij"
    );
    // Adjacent: 2..4 and 4..6 touch; no secret byte may survive between them.
    assert_eq!(
        redact(text, &[custom(2..4), custom(4..6)]),
        "ab[REDACTED]ghij"
    );
    // Unsorted input.
    assert_eq!(
        redact(text, &[custom(7..9), custom(0..2)]),
        "[REDACTED]cdefg[REDACTED]j"
    );
}

#[test]
fn redact_with_no_findings_returns_text_unchanged() {
    assert_eq!(redact("nothing to hide", &[]), "nothing to hide");
}

#[test]
fn redact_keeps_multibyte_text_around_a_secret() {
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let text = format!("กุญแจคือ {aws} นะ");
    let findings = guard().scan(&text);

    assert_eq!(redact(&text, &findings), "กุญแจคือ [REDACTED] นะ");
}

// ---------------------------------------------------------------------------
// decide
// ---------------------------------------------------------------------------

fn policy(allow_secrets: bool, force_redact: bool) -> GuardPolicy {
    GuardPolicy {
        allow_secrets,
        force_redact,
    }
}

#[test]
fn no_findings_means_send_whatever_the_policy() {
    for allow_secrets in [false, true] {
        for force_redact in [false, true] {
            for interactive in [false, true] {
                assert_eq!(
                    decide(&[], policy(allow_secrets, force_redact), interactive),
                    Decision::Send
                );
            }
        }
    }
}

#[test]
fn findings_are_decided_by_policy_then_by_interactivity() {
    let found = [custom(0..4)];

    // force_redact wins, even over allow_secrets.
    for allow_secrets in [false, true] {
        for interactive in [false, true] {
            assert_eq!(
                decide(&found, policy(allow_secrets, true), interactive),
                Decision::Redact
            );
        }
    }
    // allow_secrets sends as-is.
    for interactive in [false, true] {
        assert_eq!(
            decide(&found, policy(true, false), interactive),
            Decision::Send
        );
    }
    // Otherwise: ask on a TTY, block in a pipe or script.
    assert_eq!(
        decide(&found, policy(false, false), true),
        Decision::AskUser
    );
    assert_eq!(decide(&found, policy(false, false), false), Decision::Block);
}

// ---------------------------------------------------------------------------
// must-not-detect
// ---------------------------------------------------------------------------

fn assert_clean(text: &str) {
    let findings = guard().scan(text);
    assert!(
        findings.is_empty(),
        "false positive in {text:?}: {findings:?}"
    );
}

#[test]
fn ignores_ordinary_words_containing_sk() {
    assert_clean("The skeleton key opened the risk-free skylight.");
    assert_clean("Ask Dave. Task-switching is a sk-ill, see sk-learn and sk-12345.");
}

#[test]
fn ignores_pem_private_key_header_without_a_body() {
    let header = "-----BEGIN RSA PRIVATE KEY-----";
    // In documentation, on its own line, followed by prose.
    assert_clean(&format!(
        "A key file starts with\n{header}\nand everything after it must stay secret."
    ));
    // Inline in a sentence.
    assert_clean(&format!("Look for {header} at the top of the file."));
    // At the very end of the text.
    assert_clean(&format!("The header looks like this:\n{header}"));
}

#[test]
fn ignores_long_kebab_case_names_starting_with_sk() {
    assert_clean("kubectl delete deploy sk-my-really-long-kubernetes-resource-name-for-production");
    assert_clean("bucket sk_snake_case_name_that_is_long_enough_to_look_like_a_key_0123");
}

#[test]
fn ignores_git_sha() {
    assert_clean("Fixed in commit 3f786850e387550fdab836ed7e6dc881de23001b.");
}

#[test]
fn ignores_uuid() {
    assert_clean("request id 123e4567-e89b-12d3-a456-426614174000");
}

#[test]
fn ignores_base64_image_fragment() {
    assert_clean(
        "<img src=\"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==\">",
    );
}

#[test]
fn ignores_akia_on_its_own() {
    assert_clean("AKIA");
    assert_clean("The prefix AKIA marks an AWS access key id.");
    // Too short to be a key.
    assert_clean(&format!("{}{}", "AKIA", "FAKE1234"));
}

#[test]
fn ignores_prefixes_without_a_token_body() {
    assert_clean("Tokens start with ghp_ or github_pat_; Slack ones with xoxb-.");
    assert_clean("Google keys begin with AIza and JWTs with eyJ.");
    assert_clean("-----BEGIN PUBLIC KEY-----\nNOTSECRET\n-----END PUBLIC KEY-----");
}
