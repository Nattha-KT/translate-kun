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

/// AWS's own documented example secret key: 40 characters.
fn fake_aws_secret() -> String {
    format!("{}{}", "wJalrXUtnFEMI/K7MDENG/", "bPxRfiCYEXAMPLEKEY")
}

#[test]
fn redact_hides_both_halves_of_an_aws_credentials_block() {
    let id = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let secret = fake_aws_secret();
    assert_eq!(secret.len(), 40);
    let text = format!(
        "[default]\naws_access_key_id = {id}\naws_secret_access_key = {secret}\nregion = eu-west-1\n"
    );

    let findings = guard().scan(&text);
    let kinds: Vec<SecretKind> = findings.iter().map(|f| f.kind).collect();
    let redacted = redact(&text, &findings);

    assert_eq!(
        kinds,
        vec![SecretKind::AwsAccessKey, SecretKind::AwsSecretAccessKey]
    );
    assert!(!redacted.contains(&id), "{redacted}");
    assert!(!redacted.contains(&secret), "{redacted}");
    assert_eq!(
        redacted,
        "[default]\naws_access_key_id = [REDACTED]\naws_secret_access_key = [REDACTED]\nregion = eu-west-1\n"
    );
}

#[test]
fn detects_aws_secret_access_key_in_common_assignment_forms() {
    let secret = fake_aws_secret();
    for before in [
        "AWS_SECRET_ACCESS_KEY=",
        "export AWS_SECRET_ACCESS_KEY=\"",
        "aws_secret_access_key: '",
        "\"SecretAccessKey\": \"",
        "secretAccessKey=",
    ] {
        assert_found_once(before, &secret, "\"\n", SecretKind::AwsSecretAccessKey);
    }
    // At the very end of the text, and JSON-escaped.
    assert_found_once(
        "aws_secret_access_key=",
        &secret,
        "",
        SecretKind::AwsSecretAccessKey,
    );
    assert_found_once(
        r#"{"cfg": "aws_secret_access_key = "#,
        &secret,
        r#"\nregion = x"}"#,
        SecretKind::AwsSecretAccessKey,
    );
}

#[test]
fn ignores_aws_secret_key_names_without_a_40_character_value() {
    assert_clean("Put aws_secret_access_key in ~/.aws/credentials.");
    assert_clean("aws_secret_access_key = <your secret here>");
    assert_clean("AWS_SECRET_ACCESS_KEY=FAKEshort123");
    // 41 characters is not an AWS secret key.
    assert_clean(&format!("AWS_SECRET_ACCESS_KEY={}X", fake_aws_secret()));
    // Without a key name, a 40-character base64 run is not reported.
    assert_clean(&format!("value {}", fake_aws_secret()));
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

#[test]
fn detects_stripe_live_secret_and_restricted_keys() {
    for prefix in ["sk_live_", "rk_live_"] {
        let key = format!("{prefix}{}", "FAKEEXAMPLE0123456789abcd");
        assert_found_once("STRIPE_KEY=", &key, "\n", SecretKind::StripeKey);
    }
}

#[test]
fn ignores_stripe_test_keys_and_bare_prefixes() {
    assert_clean(&format!("{}{}", "sk_test_", "FAKEEXAMPLE0123456789abcd"));
    assert_clean("Live keys start with sk_live_ or rk_live_.");
    assert_clean(&format!("{}{}", "sk_live_", "FAKE123"));
    assert_clean(&format!("{}{}", "task_live_", "FAKEEXAMPLE0123456789abcd"));
}

#[test]
fn detects_gitlab_personal_access_token() {
    let token = format!("{}{}", "glpat-", "FAKE-EXAMPLE_0123456");
    assert_found_once("token: ", &token, "\n", SecretKind::GitLabToken);
}

#[test]
fn ignores_gitlab_prefix_without_a_token_body() {
    assert_clean("GitLab tokens begin with glpat- and then 20 characters.");
    assert_clean(&format!("{}{}", "glpat-", "FAKE0123"));
}

#[test]
fn detects_npm_access_token() {
    let token = format!("{}{}", "npm_", "FAKEEXAMPLE0123456789abcdefghijklmno");
    assert_eq!(token.len(), 4 + 36);
    assert_found_once(
        "//registry.npmjs.org/:_authToken=",
        &token,
        "\n",
        SecretKind::NpmToken,
    );
}

#[test]
fn ignores_npm_environment_variables_and_short_tokens() {
    assert_clean("echo $npm_config_registry $npm_package_version $npm_lifecycle_event");
    assert_clean(&format!("{}{}", "npm_", "FAKE0123456789"));
    // 37 characters is not an npm token.
    assert_clean(&format!(
        "{}{}",
        "npm_", "FAKEEXAMPLE0123456789abcdefghijklmnoX"
    ));
}

#[test]
fn detects_hugging_face_token() {
    let token = format!("{}{}", "hf_", "FAKEEXAMPLE0123456789abcdefghijklm");
    assert_eq!(token.len(), 3 + 34);
    assert_found_once("HF_TOKEN=", &token, "\n", SecretKind::HuggingFaceToken);
}

#[test]
fn ignores_hugging_face_function_names_and_short_values() {
    assert_clean("from huggingface_hub import hf_hub_download, hf_hub_url");
    assert_clean("export HF_HOME=~/.cache/hf_cache_directory_for_models_and_data");
    assert_clean(&format!("{}{}", "hf_", "FAKE0123456789"));
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

/// Writes newlines the way JSON does: as the two characters `\n`.
fn json_escaped(text: &str) -> String {
    text.replace('\n', r"\n")
}

/// Checks that `block` is found exactly once, as a private key block, in
/// `before + block + after`, both with real newlines and JSON-escaped, and
/// that redacting leaves `before` and `after` untouched.
fn assert_pem_found(before: &str, block: &str, after: &str) {
    let plain = (before.to_string(), block.to_string(), after.to_string());
    let escaped = (
        json_escaped(before),
        json_escaped(block),
        json_escaped(after),
    );
    for (before, block, after) in [plain, escaped] {
        let text = format!("{before}{block}{after}");

        let findings = guard().scan(&text);

        assert_eq!(findings.len(), 1, "in {text:?}");
        assert_eq!(findings[0].kind, SecretKind::PrivateKeyBlock);
        assert_eq!(&text[findings[0].range.clone()], block);
        assert_eq!(
            redact(&text, &findings),
            format!("{before}[REDACTED]{after}")
        );
    }
}

/// Checks that `text` has no finding, with real newlines and JSON-escaped.
fn assert_pem_clean(text: &str) {
    for text in [text.to_string(), json_escaped(text)] {
        let findings = guard().scan(&text);
        assert!(findings.is_empty(), "false positive in {text:?}");
    }
}

#[test]
fn encrypted_pem_without_end_is_fully_covered() {
    let block = format!(
        "{}{}{}{}",
        "-----BEGIN RSA PRIVATE KEY-----\n",
        "Proc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,FAKE0123\n\n",
        "FAKEEXAMPLEFAKEEXAMPLE0123456789\n",
        "Zg=="
    );
    assert_pem_found("my key:\n", &block, "\n\nThanks!");
    assert_pem_found("my key:\n", &block, "");
}

#[test]
fn truncated_pem_with_short_last_line_leaves_nothing_behind() {
    let block = format!(
        "{}{}{}",
        "-----BEGIN PRIVATE KEY-----\n", "FAKEEXAMPLEFAKEEXAMPLE0123456789\n", "Zg=="
    );
    assert_pem_found("", &block, "");
    assert_pem_found("", &block, "\n\nNext paragraph.");
}

#[test]
fn whole_pem_block_is_found_with_json_escaped_newlines_too() {
    assert_pem_found("before\n", &fake_pem("EC "), "\nafter");
}

#[test]
fn whole_pem_block_with_blank_line_inside_is_fully_covered() {
    let dashes = "-----";
    let block = format!(
        "{dashes}BEGIN PRIVATE KEY{dashes}\nFAKEEXAMPLEFAKEEXAMPLE0123456789\n\nNOTAREALKEYNOTAREALKEY==\n{dashes}END PRIVATE KEY{dashes}"
    );
    // Checked with real and JSON-escaped newlines; nothing between BEGIN and
    // END may survive redaction.
    assert_pem_found("before\n", &block, "\nafter");
}

#[test]
fn truncated_pem_does_not_swallow_notes_after_it() {
    let block = format!(
        "{}{}",
        "-----BEGIN PRIVATE KEY-----\n", "FAKEEXAMPLEFAKEEXAMPLE0123456789"
    );
    assert_pem_found("", &block, "\nNotes: see below\n\nThanks");
}

#[test]
fn pem_header_in_docs_does_not_reach_into_a_later_key() {
    let dashes = "-----";
    let doc = format!("A key starts with\n{dashes}BEGIN PRIVATE KEY{dashes}\nlike this one:\n");
    assert_pem_found(&doc, &fake_pem(""), "\nafter");
}

#[test]
fn pem_header_and_end_with_only_metadata_is_not_a_finding() {
    let dashes = "-----";
    assert_pem_clean(&format!(
        "{dashes}BEGIN RSA PRIVATE KEY{dashes}\nProc-Type: 4,ENCRYPTED\n\n{dashes}END RSA PRIVATE KEY{dashes}"
    ));
    assert_pem_clean(&format!(
        "{dashes}BEGIN PRIVATE KEY{dashes}\n{dashes}END PRIVATE KEY{dashes}"
    ));
}

#[test]
fn pem_header_in_prose_is_not_a_finding() {
    let header = "-----BEGIN RSA PRIVATE KEY-----";
    assert_pem_clean(&format!("Look for {header} at the top of the file."));
    assert_pem_clean(&format!("It starts with\n{header}\nand then the key."));
    assert_pem_clean(&format!(
        "It starts with\n{header}\n\nFAKEEXAMPLE0123456789"
    ));
    assert_pem_clean(&format!("The header is:\n{header}"));
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

#[test]
fn detects_jwt_with_short_signature() {
    let jwt = format!("{}.{}.{}", "eyJhbGciOiJIUzI1NiJ9", "e30", "FAKE01");
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
            SecretKind::StripeKey,
            format!("{}{}", "sk_live_", "FAKEEXAMPLE0123456789abcd"),
        ),
        (
            SecretKind::GitLabToken,
            format!("{}{}", "glpat-", "FAKE-EXAMPLE_0123456"),
        ),
        (
            SecretKind::NpmToken,
            format!("{}{}", "npm_", "FAKEEXAMPLE0123456789abcdefghijklmno"),
        ),
        (
            SecretKind::HuggingFaceToken,
            format!("{}{}", "hf_", "FAKEEXAMPLE0123456789abcdefghijklm"),
        ),
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

/// Checks that `secret` is found exactly once, with the reported range
/// covering only the secret, in `before + secret + after`.
fn assert_found_once(before: &str, secret: &str, after: &str, kind: SecretKind) {
    let text = format!("{before}{secret}{after}");

    let findings = guard().scan(&text);

    assert_eq!(findings.len(), 1, "{kind:?} not found once in {text:?}");
    assert_eq!(findings[0].kind, kind);
    assert_eq!(&text[findings[0].range.clone()], secret, "{kind:?}");
}

#[test]
fn detects_a_key_after_a_json_escaped_newline() {
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    assert_found_once(r"a\n", &aws, "", SecretKind::AwsAccessKey);
}

#[test]
fn detects_a_key_after_a_json_escaped_tab() {
    let google = format!("{}{}", "AIza", "SyFAKE-EXAMPLE_0123456789abcdefghij");
    assert_found_once(r"x\t", &google, "", SecretKind::GoogleApiKey);
}

#[test]
fn detects_a_key_after_an_underscore() {
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    assert_found_once("KEY_", &aws, "", SecretKind::AwsAccessKey);
}

#[test]
fn detects_a_github_token_inside_a_json_string_after_an_escaped_newline() {
    let token = format!("{}{}", "ghp_", "FAKEexample0123456789abcdefghijklmno");
    assert_found_once(
        r#"{"notes": "line one\n"#,
        &token,
        r#"\nline three"}"#,
        SecretKind::GitHubToken,
    );
}

#[test]
fn detects_every_prefixed_kind_after_escapes_and_underscores() {
    for (kind, secret) in one_sample_per_kind() {
        if kind == SecretKind::PrivateKeyBlock || kind == SecretKind::OpenAiApiKey {
            continue;
        }
        for before in [r"a\n", r"x\t", "KEY_", "\"", "("] {
            assert_found_once(before, &secret, "", kind);
        }
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
    let Err(error) = SecretGuard::new(&[leaky]) else {
        panic!("expected an error for an invalid pattern")
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
    // First segment is 40+ letters/digits, but the name goes on with `-`/`_`.
    let segment = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEF";
    assert!(segment.len() >= 40);
    assert_clean(&format!("deploy sk-{segment}-more-words"));
    assert_clean(&format!("deploy sk-{segment}_more_words"));
}

#[test]
fn ignores_prefixes_glued_to_the_end_of_a_longer_word() {
    // The prefix must follow a separator, not a letter or digit.
    assert_clean("deploy task-ant-migration-runner-for-production-cluster");
    assert_clean(&format!("{}{}", "XAKIA", "IOSFODNN7EXAMPLE"));
    assert_clean(&format!(
        "{}{}",
        "BAIza", "SyFAKE-EXAMPLE_0123456789abcdefghij"
    ));
    assert_clean(&format!(
        "{}{}",
        "highp_", "FAKEexample0123456789abcdefghijklmno"
    ));
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
