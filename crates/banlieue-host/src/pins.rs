// Copyright (c) 2026 Erick Bourgeois, banlieue
// SPDX-License-Identifier: Apache-2.0
//! The one pinned VMM release (ADR-0067 Decision 4).
//!
//! The version must equal the release the client's vendored REST spec is
//! pinned to (ADR-0061 Decision 2); `pins_tests.rs` asserts it. There is no
//! flag to install anything else: changing the pin is a code change, with
//! new digests and that test.

use crate::paths::{BIN_DIR, OPT_ROOT};
use std::path::PathBuf;

/// The Cloud Hypervisor release.
pub const VMM_VERSION: &str = "v53.0";
/// sha256 of that release's `cloud-hypervisor-static`.
pub const VMM_SHA256: &str = "448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc";
/// sha256 of that release's `ch-remote-static`.
pub const CH_REMOTE_SHA256: &str =
    "13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7";
/// The Cloud Hypervisor edk2 firmware release.
pub const FIRMWARE_TAG: &str = "ch-97eeb7b09";
/// sha256 of that release's `CLOUDHV.fd`.
pub const FIRMWARE_SHA256: &str =
    "dc2fc8f0e43b96712d9fccc52a3a590769606412b3e1bc911d217addd3bef624";

const VMM_RELEASES: &str = "https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download";
const FIRMWARE_RELEASES: &str = "https://github.com/cloud-hypervisor/edk2/releases/download";

/// Executables.
const MODE_EXECUTABLE: u32 = 0o755;
/// Firmware: read by every guest's VMM.
const MODE_FIRMWARE: u32 = 0o644;

/// One pinned file: where it comes from, what it must hash to, where it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    /// The upstream release asset's name, also its name in `--artifacts-dir`.
    pub name: &'static str,
    /// Where to download it.
    pub url: String,
    /// Its sha256, lowercase hex.
    pub sha256: String,
    /// Where it is installed.
    pub dest: PathBuf,
    /// Its mode once installed.
    pub mode: u32,
}

/// The versioned directory holding the VMM and `ch-remote`.
#[must_use]
pub fn vmm_dir() -> PathBuf {
    PathBuf::from(OPT_ROOT)
        .join("cloud-hypervisor")
        .join(VMM_VERSION)
}

/// The versioned directory holding the firmware.
#[must_use]
pub fn firmware_dir() -> PathBuf {
    PathBuf::from(OPT_ROOT).join("firmware").join(FIRMWARE_TAG)
}

/// The installed firmware, as the host config names it.
#[must_use]
pub fn firmware_path() -> PathBuf {
    firmware_dir().join("CLOUDHV.fd")
}

/// Everything the `vmm` stage installs, in order.
#[must_use]
pub fn artifacts() -> Vec<Artifact> {
    vec![
        Artifact {
            name: "cloud-hypervisor-static",
            url: format!("{VMM_RELEASES}/{VMM_VERSION}/cloud-hypervisor-static"),
            sha256: VMM_SHA256.into(),
            dest: vmm_dir().join("cloud-hypervisor"),
            mode: MODE_EXECUTABLE,
        },
        Artifact {
            name: "ch-remote-static",
            url: format!("{VMM_RELEASES}/{VMM_VERSION}/ch-remote-static"),
            sha256: CH_REMOTE_SHA256.into(),
            dest: vmm_dir().join("ch-remote"),
            mode: MODE_EXECUTABLE,
        },
        Artifact {
            name: "CLOUDHV.fd",
            url: format!("{FIRMWARE_RELEASES}/{FIRMWARE_TAG}/CLOUDHV.fd"),
            sha256: FIRMWARE_SHA256.into(),
            dest: firmware_path(),
            mode: MODE_FIRMWARE,
        },
    ]
}

/// The command symlinks, `(link, target)`, set once every artifact verified.
#[must_use]
pub fn symlinks() -> Vec<(PathBuf, PathBuf)> {
    vec![
        (
            PathBuf::from(BIN_DIR).join("cloud-hypervisor"),
            vmm_dir().join("cloud-hypervisor"),
        ),
        (
            PathBuf::from(BIN_DIR).join("ch-remote"),
            vmm_dir().join("ch-remote"),
        ),
    ]
}

/// What the `vmm` stage installs and the self-test checks: the pinned
/// release in production, a release of known bytes in the unit tests (no
/// test can produce bytes matching the real digests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The files, fetched and verified before any is installed.
    pub artifacts: Vec<Artifact>,
    /// `(link, target)`, set once every artifact verified.
    pub symlinks: Vec<(PathBuf, PathBuf)>,
    /// The installed firmware.
    pub firmware: PathBuf,
    /// Its sha256.
    pub firmware_sha256: String,
}

impl Release {
    /// The one pinned release (ADR-0067 Decision 4).
    #[must_use]
    pub fn pinned() -> Self {
        Self {
            artifacts: artifacts(),
            symlinks: symlinks(),
            firmware: firmware_path(),
            firmware_sha256: FIRMWARE_SHA256.into(),
        }
    }
}

/// Lowercase hex sha256 of `data`.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod pins_tests;
