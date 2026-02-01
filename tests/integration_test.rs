use launch_structure_verifier::*;
use launch_structure_verifier::checks::*;
use launch_structure_verifier::scoring::aggregate_score;

#[test]
fn test_fair_launch_solana_full_flow() {
    // Simulate the "fair launch" golden fixture
    let facts = TokenFacts {
        metadata: Some(Metadata {
            name: Some("FairToken".to_string()),
            symbol: Some("FAIR".to_string()),
            decimals: Some(9),
            standard: TokenStandard::SplToken,
        }),
        supply: Some(SupplyInfo {
            total_supply_raw: Some("1000000000000000".to_string()),
            total_supply: Some(1000000.0),
        }),
        authorities: Some(AuthorityInfo {
            mint_authority: None,
            freeze_authority: None,
            owner: None,
            mint_mutable: Some(false),
        }),
        holders: Some(HolderInfo {
            top1_pct: Some(8.5),
            top5_pct: Some(28.0),
            top_holders: vec![],
        }),
        creation: Some(CreationInfo {
            created_at: Some("2026-01-20T00:00:00Z".to_string()),
            age_seconds: Some(864000),
            age_band: AgeBand::GreaterThan7d,
        }),
    };

    // Run all 6 checks
    let checks = vec![
        check_mint_authority_disabled(&facts),
        check_freeze_authority_disabled(&facts),
        check_holder_concentration(&facts),
        check_token_age(&facts),
        check_standard_sanity(&facts, "solana"),
    ];

    // Aggregate score
    let result = aggregate_score(&checks);

    // Assertions
    assert!(result.fairness_score.unwrap() >= 95, "Fair launch should score very high");
    assert!(matches!(result.grade, Grade::Strong));
    assert_eq!(result.weights_total, 85); // All checks present except ownership (EVM-only)
    
    // Verify no critical failures
    for check in &checks {
        if matches!(check.severity, Severity::Critical) {
            assert!(matches!(check.status, CheckStatus::Pass), 
                "Critical check {} should pass", check.id);
        }
    }
}

#[test]
fn test_mint_authority_exists_critical_override() {
    // Simulate the "mint authority exists" fixture
    let facts = TokenFacts {
        metadata: Some(Metadata {
            name: Some("UnfairToken".to_string()),
            symbol: Some("UNFAIR".to_string()),
            decimals: Some(9),
            standard: TokenStandard::SplToken,
        }),
        supply: Some(SupplyInfo {
            total_supply_raw: Some("1000000000000000".to_string()),
            total_supply: Some(1000000.0),
        }),
        authorities: Some(AuthorityInfo {
            mint_authority: Some("SomeAuthorityKey123".to_string()),
            freeze_authority: None,
            owner: None,
            mint_mutable: Some(true),
        }),
        holders: Some(HolderInfo {
            top1_pct: Some(5.0),
            top5_pct: Some(20.0),
            top_holders: vec![],
        }),
        creation: Some(CreationInfo {
            created_at: Some("2026-01-20T00:00:00Z".to_string()),
            age_seconds: Some(864000),
            age_band: AgeBand::GreaterThan7d,
        }),
    };

    // Run all checks
    let checks = vec![
        check_mint_authority_disabled(&facts),
        check_freeze_authority_disabled(&facts),
        check_holder_concentration(&facts),
        check_token_age(&facts),
        check_standard_sanity(&facts, "solana"),
    ];

    // Aggregate score
    let result = aggregate_score(&checks);

    // Critical assertion: grade MUST be Compromised
    assert!(matches!(result.grade, Grade::Compromised), 
        "Mint authority exists should force Compromised grade");
    
    // Score might be high mathematically, but grade is overridden
    println!("Score: {:?}, Grade: {:?}", result.fairness_score, result.grade);
}

#[test]
fn test_evm_fair_launch() {
    let facts = TokenFacts {
        metadata: Some(Metadata {
            name: Some("FairERC".to_string()),
            symbol: Some("FERC".to_string()),
            decimals: Some(18),
            standard: TokenStandard::Erc20,
        }),
        supply: Some(SupplyInfo {
            total_supply_raw: Some("1000000000000000000000000".to_string()),
            total_supply: Some(1000000.0),
        }),
        authorities: Some(AuthorityInfo {
            mint_authority: None,
            freeze_authority: None,
            owner: Some("0x0000000000000000000000000000000000000000".to_string()),
            mint_mutable: Some(false),
        }),
        holders: Some(HolderInfo {
            top1_pct: Some(9.0),
            top5_pct: Some(33.0),
            top_holders: vec![],
        }),
        creation: Some(CreationInfo {
            created_at: Some("2026-01-20T00:00:00Z".to_string()),
            age_seconds: Some(864000),
            age_band: AgeBand::GreaterThan7d,
        }),
    };

    let checks = vec![
        check_ownership_renounced(&facts),
        check_holder_concentration(&facts),
        check_token_age(&facts),
        check_standard_sanity(&facts, "evm"),
    ];

    let result = aggregate_score(&checks);

    assert!(result.fairness_score.unwrap() >= 95);
    assert!(matches!(result.grade, Grade::Strong));
}

#[test]
fn test_partial_data_realistic_scenario() {
    // Realistic: provider timeout on holder data
    let facts = TokenFacts {
        metadata: Some(Metadata {
            name: Some("PartialToken".to_string()),
            symbol: Some("PART".to_string()),
            decimals: Some(9),
            standard: TokenStandard::SplToken,
        }),
        supply: Some(SupplyInfo {
            total_supply: Some(1000000.0),
            total_supply_raw: Some("1000000000000000".to_string()),
        }),
        authorities: Some(AuthorityInfo {
            mint_authority: None,
            freeze_authority: None,
            owner: None,
            mint_mutable: Some(false),
        }),
        holders: None, // Provider timeout
        creation: Some(CreationInfo {
            age_seconds: Some(259200),
            created_at: Some("2026-01-27T00:00:00Z".to_string()),
            age_band: AgeBand::Day1To7,
        }),
    };

    let checks = vec![
        check_mint_authority_disabled(&facts),
        check_freeze_authority_disabled(&facts),
        check_holder_concentration(&facts), // Will return Unknown
        check_token_age(&facts),
        check_standard_sanity(&facts, "solana"),
    ];

    let result = aggregate_score(&checks);

    // Holder concentration should be unknown
    let holder_check = checks.iter().find(|c| c.id == "holder_concentration").unwrap();
    assert!(matches!(holder_check.status, CheckStatus::Unknown));
    assert_eq!(holder_check.score_component, None);

    // Weight should exclude holder concentration (20)
    assert_eq!(result.weights_total, 65); // 85 - 20 = 65

    // Grade should still be Strong because structure is sound
    assert!(matches!(result.grade, Grade::Strong));
}
