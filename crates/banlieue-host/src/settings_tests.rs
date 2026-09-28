// Copyright (c) 2026 Erick Bourgeois, banlieue
// SPDX-License-Identifier: Apache-2.0
//! Unit tests for `settings.rs`.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::fake::FakeHost;
    use crate::ops::Kind;
    use clap::Parser;

    #[derive(Parser)]
    struct T {
        #[command(flatten)]
        args: HostArgs,
    }

    fn args(argv: &[&str]) -> HostArgs {
        T::parse_from(std::iter::once("t").chain(argv.iter().copied())).args
    }

    /// A bare host: storage on the roomiest candidate, the libvirt bridge
    /// if there is one, the host's name, the documented uid range.
    #[test]
    fn defaults_come_from_the_host() {
        let h = FakeHost::debian();
        h.put_dir("/data");
        {
            let mut env = h.env.lock().unwrap();
            env.free.insert("/data".into(), 900 << 30);
        }
        let s = resolve(&args(&[]), &h).unwrap();
        assert_eq!(s.provider_name, "bar");
        assert_eq!(s.namespace, DEFAULT_NAMESPACE);
        assert_eq!(
            s.storage,
            vec![("default".into(), PathBuf::from("/data/banlieue/ch"))]
        );
        assert_eq!(s.network, vec![("default".into(), "virbr0".into())]);
        assert_eq!(
            (s.uid_base, s.uid_count),
            (DEFAULT_GUEST_UID_BASE, DEFAULT_GUEST_UID_COUNT)
        );
        assert_eq!(
            s.uid_end(),
            DEFAULT_GUEST_UID_BASE + DEFAULT_GUEST_UID_COUNT - 1
        );
        assert!(s.registry.is_none());
    }

    #[test]
    fn no_bridge_means_no_default_network_class() {
        let h = FakeHost::debian();
        h.state
            .lock()
            .unwrap()
            .fs
            .retain(|p, _| !p.starts_with("/sys/class/net"));
        assert!(resolve(&args(&[]), &h).unwrap().network.is_empty());
    }

    #[test]
    fn classes_are_taken_in_order_from_flags_or_a_list() {
        let h = FakeHost::debian();
        let s = resolve(
            &args(&[
                "--storage-class",
                "fast=/nvme/ch,slow=/srv/ch",
                "--network-class",
                "lan=br0",
                "--provider-name",
                "baz",
                "--registry-repository",
                "registry.internal:5000/banlieue/disks",
            ]),
            &h,
        )
        .unwrap();
        assert_eq!(
            s.storage,
            vec![
                ("fast".into(), PathBuf::from("/nvme/ch")),
                ("slow".into(), PathBuf::from("/srv/ch"))
            ]
        );
        assert_eq!(s.network, vec![("lan".into(), "br0".into())]);
        assert_eq!(s.provider_name, "baz");
        let r = s.registry.unwrap();
        assert_eq!(r.repository, "registry.internal:5000/banlieue/disks");
        assert_eq!(r.keep_unreferenced, DEFAULT_KEEP_UNREFERENCED);
        assert!(!r.plain_http);
    }

    /// Values end up in a TOML file and in unit files: anything that could
    /// break either is refused, not quoted.
    #[test]
    fn unusable_values_are_refused() {
        let h = FakeHost::debian();
        for bad in [
            vec!["--storage-class", "nopath"],
            vec!["--storage-class", "default=relative/dir"],
            vec!["--storage-class", "Upper=/srv/x"],
            vec!["--storage-class", "a=/srv/x,a=/srv/y"],
            vec!["--network-class", "lan=br 0"],
            vec!["--network-class", "lan=br\"0"],
            vec!["--guest-uid-count", "0"],
            vec!["--guest-uid-base", "4294967295", "--guest-uid-count", "2"],
        ] {
            assert!(
                matches!(resolve(&args(&bad), &h), Err(Error::Setting { .. })),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_default_storage_falls_back_to_var_lib() {
        let h = FakeHost::default();
        h.put_file("/sys/class/net/eth0", b"", Kind::Other);
        assert_eq!(default_storage(&h), PathBuf::from("/var/lib/banlieue/ch"));
    }
}
