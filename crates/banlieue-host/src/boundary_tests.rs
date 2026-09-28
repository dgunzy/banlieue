// Copyright (c) 2026 Erick Bourgeois, banlieue
// SPDX-License-Identifier: Apache-2.0
//! ADR-0067 Decision 3 / roadmap 09 phase 10, invariant 7: no reconcile path
//! can reach the installer, because nothing a reconciler links depends on
//! this crate.

#[cfg(test)]
mod tests {
    const PROVIDER_MANIFEST: &str =
        include_str!("../../banlieue-provider-cloud-hypervisor/Cargo.toml");
    const CLIENT_MANIFEST: &str = include_str!("../../banlieue-cloud-hypervisor/Cargo.toml");
    const SDK_MANIFEST: &str = include_str!("../../banlieue-provider-sdk/Cargo.toml");

    #[test]
    fn no_reconciler_crate_depends_on_the_installer() {
        for (name, manifest) in [
            ("banlieue-provider-cloud-hypervisor", PROVIDER_MANIFEST),
            ("banlieue-cloud-hypervisor", CLIENT_MANIFEST),
            ("banlieue-provider-sdk", SDK_MANIFEST),
        ] {
            assert!(
                !manifest.contains("banlieue-host"),
                "{name} depends on banlieue-host"
            );
        }
    }
}
