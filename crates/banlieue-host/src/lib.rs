// Copyright (c) 2026 Erick Bourgeois, banlieue
// SPDX-License-Identifier: Apache-2.0
//! # banlieue-host
//!
//! `banlieue host`: prepare a KVM host for banlieue's Cloud Hypervisor
//! provider from the binary itself (ADR-0067), the way `k0s install`
//! prepares a node. It replaces the body of
//! `scripts/bootstrap-cloud-hypervisor-host.sh`, which is now a thin
//! `--remote` wrapper around this.
//!
//! ```text
//! banlieue host preflight          # changes nothing
//! banlieue host status             # changes nothing
//! banlieue host selftest           # changes nothing, boots nothing
//! banlieue host install            # every stage, in order
//! banlieue host install --only tpm
//! banlieue host install --dry-run
//! ```
//!
//! This crate is the installer, not a controller. It runs once, as root, at
//! an operator's request, and may therefore run `apt-get`, `useradd`,
//! `systemctl` and `swtpm_setup`: ADR-0011 forbids subprocesses in a
//! reconcile path, and no reconcile path can reach this crate, because the
//! provider crate does not depend on it (`boundary_tests.rs`).

pub mod dryrun;
pub mod error;
#[cfg(test)]
pub mod fake;
pub mod fetch;
pub mod ops;
pub mod paths;
pub mod pins;
pub mod real;
pub mod render;
pub mod settings;
pub mod stages;

pub use error::Error;

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// `banlieue host <verb>`.
#[derive(Debug, Args)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub verb: Verb,
}

/// The verbs. Only `install` changes the host.
#[derive(Debug, Subcommand)]
pub enum Verb {
    /// Is this host fit for guests: bare metal, /dev/kvm, bridges, a free
    /// guest uid range, NSS. Changes nothing.
    Preflight(settings::HostArgs),
    /// Report what is installed. Changes nothing.
    Status(settings::HostArgs),
    /// Prove the pieces work together: the pinned VMM and firmware, guest
    /// uids, /dev/kvm, the provider's own host checks, a vTPM with the
    /// right EK CN. Boots nothing, leaves nothing. Needs root.
    Selftest(settings::HostArgs),
    /// Prepare this host, every stage in order, or one with --only. Needs
    /// root, except with --dry-run.
    Install(InstallArgs),
}

/// `banlieue host install`.
#[derive(Debug, Args)]
pub struct InstallArgs {
    /// The host's settings.
    #[command(flatten)]
    pub settings: settings::HostArgs,
    /// Run one stage; its prerequisites must already hold.
    #[arg(long, value_enum)]
    pub only: Option<stages::Stage>,
    /// Install missing packages with apt-get (default: verify, and list
    /// what is missing).
    #[arg(long, env = "BANLIEUE_HOST_INSTALL_PACKAGES")]
    pub install_packages: bool,
    /// Regenerate the host config and ROTATE THE EK CA: every EK
    /// certificate this host issued stops verifying.
    #[arg(long)]
    pub force: bool,
    /// Take the pinned release assets (`cloud-hypervisor-static`,
    /// `ch-remote-static`, `CLOUDHV.fd`) from this directory instead of
    /// downloading them. They are verified the same way.
    #[arg(long, env = "BANLIEUE_HOST_ARTIFACTS_DIR")]
    pub artifacts_dir: Option<PathBuf>,
    /// Install this file as the provider binary (default: leave it).
    #[arg(long, env = "BANLIEUE_HOST_PROVIDER_BINARY")]
    pub provider_binary: Option<PathBuf>,
    /// Print what would change, change nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Run a `banlieue host` verb on this host.
///
/// # Errors
/// The verb's [`Error`].
pub async fn run(cli: Cli) -> Result<(), Error> {
    let host = real::RealHost::new();
    let facts = |c: &banlieue_provider_cloud_hypervisor::host_config::HostConfig| {
        banlieue_provider_cloud_hypervisor::provider::gather_facts(c)
    };
    match cli.verb {
        Verb::Preflight(args) => stages::preflight(&host, &settings::resolve(&args, &host)?),
        Verb::Status(args) => {
            settings::resolve(&args, &host)?;
            stages::status(&host);
            Ok(())
        }
        Verb::Selftest(args) => {
            let s = settings::resolve(&args, &host)?;
            if !ops::Probe::is_root(&host) {
                return Err(Error::NotRoot("selftest"));
            }
            stages::selftest(&host, &pins::Release::pinned(), &s, &facts)
        }
        Verb::Install(args) => {
            let s = settings::resolve(&args.settings, &host)?;
            let o = stages::Options {
                only: args.only,
                install_packages: args.install_packages,
                force: args.force,
                provider_binary: args.provider_binary,
                dry_run: args.dry_run,
            };
            let fetch: Box<dyn fetch::Fetch> = match args.artifacts_dir {
                Some(dir) => Box::new(fetch::FromDir { dir }),
                None => Box::new(fetch::Download::new()),
            };
            if o.dry_run {
                let dry = dryrun::DryRun::new(&host);
                return stages::install(
                    &dry,
                    fetch.as_ref(),
                    &pins::Release::pinned(),
                    &s,
                    &o,
                    &facts,
                )
                .await;
            }
            if !ops::Probe::is_root(&host) {
                return Err(Error::NotRoot("install"));
            }
            stages::install(
                &host,
                fetch.as_ref(),
                &pins::Release::pinned(),
                &s,
                &o,
                &facts,
            )
            .await
        }
    }
}

#[cfg(test)]
#[path = "boundary_tests.rs"]
mod boundary_tests;
