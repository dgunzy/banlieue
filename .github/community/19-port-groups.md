# 19: Port groups: declared, reconciled, and safe to delete

> **Goal.** Let an admin declare an L2 network segment once, as a banlieue
> object: VLAN, MTU, security policy, and which Providers carry it. banlieue
> then **creates, reconciles, and removes the backing port group** on each
> backend: a distributed port group on vSphere, a libvirt `<network>` (with
> `<portgroup>`s), a Proxmox SDN VNet, a VLAN on a Cloud Hypervisor host
> bridge. Today every port group is created by hand beforehand, and banlieue
> only sees its name as a string in
> `Provider.spec.capabilities.networkClasses[].target`.
>
> **Stop condition.** On vSphere and libvirt, a `VMNetwork` with VLAN 100
> produces a backing port group in every zone it targets, and a
> `networkClass` that references it resolves with no literal name in the
> `Provider`. A `VirtualMachine` on that class boots onto VLAN 100. Deleting
> the `VMNetwork` while any NIC is attached is refused. Once the NIC is gone,
> the port group is removed. A port group banlieue did **not** create is
> never modified or deleted.

Baseline when written: `ca07fb0` (2026-09-30). ADRs 0001–0067 exist or are
reserved (0057–0059 by roadmap 15, 0068–0073 by roadmap 18); 0074 and 0075
are taken by roadmap 06's Proxmox work. **This roadmap reserves ADR-0076 to
ADR-0080.** Renumber at landing if anything else claims them first.

## Why

Port groups are the one piece of backend topology banlieue still treats as
somebody else's job, and three existing ADRs route around that gap:

- **ADR-0030** gave every network class a per-zone override because "the
  same" network has a different port group name on every cluster. The
  names differ because a human made each one.
- **ADR-0032** moved gateway, DNS, and domain onto the Provider's network
  class because "a port group implies a subnet". The port group and its
  subnet are still declared in two places and kept consistent by hand.
- **ADR-0019** reports whether a class is *reachable*, which here means
  "a network with that literal name exists". A name match does not check
  the VLAN, the MTU, the switch, or whether promiscuous mode is on.

The result is a new-cluster workflow of "ask the network team for a port
group, wait, copy its name into YAML, repeat per cluster". It is also a
reachability check that stays green for a port group on the wrong VLAN.
banlieue already declares images (`VMImage`) and sizes (`VMClass`) and
reconciles them per Provider. Networks are the obvious third.

## The model

```
  VMNetwork "prod-vlan100"  (banlieue.io, cluster-scoped, admin-owned)
    spec: vlan 100, mtu 9000, securityPolicy {…}, providers [vsphere-dc1, lv-lab]
        │
        │  per Provider × per zone, like VMImage → per-zone template (ADR-0020)
        ▼
  ┌─────────────────────────────┬──────────────────────────────┬──────────────────────┐
  │ vSphere                     │ libvirt                      │ Proxmox / CH         │
  │ DVPortgroup on the zone's   │ <network> forward=bridge     │ SDN VNet (tag 100) / │
  │ vDS, VLAN 100, MTU on vDS   │ or <portgroup> on an OVS net │ VLAN on host bridge  │
  └─────────────────────────────┴──────────────────────────────┴──────────────────────┘
        │
        ▼
  VMNetwork.status.zones[]: { provider, zone, backingRef, observedVlan, ready }
        │
        ▼
  Provider.spec.capabilities.networkClasses[].vmNetworkRef: prod-vlan100
    → target resolved per zone from VMNetwork.status (no literal name, no perZone list)
```

`VMNetwork` is the declaration. The backing port group is status: a
Provider reports what it created, and the network class resolves against
that report. `status mirrors infra` (non-negotiable 6) applies here too. A
class is reachable in a zone only when the backend reports the segment
exists **with the declared VLAN**.

The literal `target: { portGroup: … }` form **stays**. Brownfield
environments with port groups owned by a network team must keep working,
and banlieue must not force a takeover. A network class names either a
literal target or a `vmNetworkRef`, never both. Admission rejects both.

## Fixed constraints (not up for re-litigation here)

| Constraint | Source |
|---|---|
| CRD-only. The main controller never talks to a backend. It writes `VMNetwork`, and each provider realizes it and reports back on status. | Non-negotiable 1 |
| Explicit over implicit. banlieue never infers a VLAN from a name or adopts a port group because it "looks right". Adoption is a declared act (phase C). | Non-negotiable 4 |
| Uniform tiering per zone: a network class available in one zone of a Provider should be available in all of them. A `VMNetwork` targets a Provider, and a missing zone is a reported failure, not a silent gap. | D-023, roadmap 03 |
| Subnet facts stay with the network class (ADR-0032). This roadmap may *move* them onto `VMNetwork`, but only by an ADR that amends 0032, and never by duplicating them. | ADR-0032 |
| Least privilege. Creating switch objects needs more backend privilege than attaching a NIC. That privilege is **opt-in per Provider**, and a Provider without it keeps working exactly as today. | Global least-privilege rule |

## Security invariants (every phase must preserve all four)

1. **banlieue deletes only what it created.** Every backing object carries
   an ownership marker: a vSphere custom attribute or tag, libvirt network
   metadata, a Proxmox VNet alias or comment. Delete and modify paths check
   that marker, and a port group without it is read-only to banlieue,
   whatever the `VMNetwork` says.
2. **No VLAN outside the Provider's allowlist.** A `VMNetwork` can bridge a
   guest onto any L2 segment the switch trunks, which crosses a network
   segmentation boundary. `Provider.spec.networking.allowedVlans` is set by
   an admin, and admission (`ValidatingAdmissionPolicy`, the ADR-0007
   pattern) rejects a `VMNetwork` whose VLAN falls outside every targeted
   Provider's list. An empty list means banlieue creates nothing.
3. **Permissive security policy is declared, never defaulted.** Promiscuous
   mode, forged transmits, and MAC changes default to *reject* on every
   backend that has the knob. Turning one on is a spec field that shows up
   in `kubectl get` output.
4. **In-use port groups are never removed.** A finalizer on `VMNetwork`
   holds deletion while any infra machine (or backend VM, for adopted
   segments) has a NIC on it. The backend check comes first: a stale
   informer cache must not let a delete through.

## ADR reservations

| ADR | Decision it will record | Phase |
|---|---|---|
| 0076 | The `VMNetwork` API: scope (cluster-scoped, admin-owned, like `VMClass`), spec (VLAN / trunk range, MTU, security policy, target Providers), status shape, `networkClasses[].vmNetworkRef` and its exclusivity with `target`, and whether ADR-0032's subnet shape moves onto `VMNetwork`. Amends ADR-0030/0032 | 0 |
| 0077 | Port group ownership, adoption, and deletion: the ownership marker per backend, the explicit `adopt` path for existing port groups, the in-use finalizer and its backend-side check, and what "never delete what we did not create" means under rename | C |
| 0078 | vSphere realization: distributed port groups on a zone's vDS (`CreateDVPortgroup_Task`), the vDS-per-zone mapping, and the separate, opt-in privileged credential (`DVPortgroup.Create/Modify/Delete`). Standard vSwitch port groups (per-host `HostNetworkSystem`) are **in or out**, decided here | B |
| 0079 | libvirt realization: a bridge-mode `<network>` per `VMNetwork` vs `<portgroup>`s on one OVS network, and how the VLAN tag lands (`<vlan>` requires OVS or macvtap passthrough; a Linux bridge needs a VLAN sub-interface the host must already have) | D |
| 0080 | Proxmox SDN VNet and Cloud Hypervisor host-bridge VLAN realization. Written with roadmap 06 (Proxmox) and against ADR-0067's `banlieue host` (Cloud Hypervisor, which owns the bridge today) | E |

## 0. Decision gate (ADR-0076)

- [ ] **Name and scope.** Proposal: `VMNetwork`, cluster-scoped, in
      `banlieue.io/v1alpha1`, next to `VMClass` and `VMImage`. "PortGroup"
      is vSphere vocabulary, and the abstract object has to read correctly
      for libvirt and Proxmox too. Per-backend realization lives in the
      provider and is reported on `VMNetwork.status.zones[]`. A new infra
      CRD per backend is the alternative and is weighed in the ADR: it
      would mirror `VSphereMachine`, but CAPI has no InfraNetwork contract to
      satisfy, so the case for one is weak.
- [ ] **Spec.** `vlan` (single ID) or `trunk` (ranges, for a guest that
      tags its own traffic), `mtu`, `securityPolicy`
      `{ promiscuous, forgedTransmits, macChanges }`, and `providers[]`
      (names). No backend-specific fields at the top level. A
      `providerOverrides[]` escape hatch exists only if the ADR can name a
      real need for one.
- [ ] **Resolution.** `NetworkClassMapping` gains `vmNetworkRef`. The
      scheduler reads the per-zone backing name from `VMNetwork.status`
      where it reads `per_zone` today. ADR-0019 reachability becomes
      "`status.zones[]` has this zone `ready=true` with `observedVlan` =
      spec".
- [ ] **Subnet placement.** Decide whether `SubnetShape` moves from the
      network class onto `VMNetwork` (a segment implies a subnet, which is
      ADR-0032's own argument) or stays put. If it moves, ADR-0076 amends
      ADR-0032 and the class-level field is removed outright. There is no
      release yet, so no deprecation window is needed (the ADR-0031
      precedent).
- [ ] **CALM.** `VMNetwork` node and its relationships to the controller,
      each provider, and the backend switch. `make calm-validate` and
      `make calm-diagrams` must pass before phase A.

## A. API and controller (backend-neutral)

- [ ] `crates/banlieue-api/src/banlieue/vmnetwork.rs` (+ `_tests.rs`),
      `regen-crds`, `examples/NN-vmnetwork-*.yaml`, and `validate-examples`.
- [ ] `Provider.spec.networking.allowedVlans` (ranges) and
      `Provider.spec.networking.manageNetworks: bool` (default `false`, which
      is the opt-in for invariant 2 and for the privileged credential).
- [ ] Admission: `deploy/admission/vmnetwork-*.yaml`. VLAN inside every
      targeted Provider's allowlist, `vlan` xor `trunk`, and `vmNetworkRef`
      xor `target` on a network class.
- [ ] Controller: resolve `vmNetworkRef` in the scheduler
      (`reconciler/scheduler.rs`) and the infra builder
      (`reconciler/infra.rs`) from `VMNetwork.status`. Unit tests beside the
      existing `("prod", "portGroup", "pg-1")` cases in `scheduler_tests.rs`.
- [ ] `VMNetwork` finalizer and its in-use index: which infra machines
      resolve a NIC to this network. Deletion waits on it (invariant 4).
- [ ] SDK: a `NetworkRealizer`-shaped trait in `banlieue-provider-sdk`,
      with the ensure/observe/delete lifecycle each provider implements. A
      fake that **rejects** what real backends reject: a duplicate name, a
      VLAN on a non-VLAN-capable switch, a delete of a port group that still
      has ports.

## B. vSphere (ADR-0078): first live target

- [ ] `client/vim.rs`: `create_dv_portgroup`, `reconfigure_dv_portgroup`,
      `destroy_dv_portgroup`, and `list_dv_portgroups` with VLAN, MTU, and
      security policy read back. `client/fake.rs` mirrors each, including
      `ResourceInUse` on destroy.
- [ ] Extend the `Network` projection (`client/mod.rs`) with `vlan`,
      `switch`, and `managedBy`. ADR-0019 reachability compares VLAN, not
      just name, for `vmNetworkRef` classes.
- [ ] Zone → vDS mapping: a zone names which vDS carries its managed port
      groups (`Provider` per-zone field, decided in 0078). A cluster whose
      hosts are not all members of that vDS is a reported failure.
- [ ] Separate privileged credential (`networkCredentialsRef`), used only
      by the network realizer. The VM-lifecycle credential keeps its
      current, smaller role. Document the vCenter role in
      `docs/src/guides/vsphere-provider.md`.
- [ ] `make vsphere-live-test` case: create → observe VLAN → attach a VM →
      delete refused → detach → delete succeeds → a pre-existing,
      unmarked port group with the same name is left untouched.

## C. Ownership, adoption, drift (ADR-0077)

- [ ] Ownership marker written at create and checked before every
      modify and delete (invariant 1). Unit-tested per backend fake.
- [ ] `VMNetwork.spec.adopt: { name }`: explicitly take over an existing
      port group. banlieue verifies VLAN and policy match the spec before
      marking it, and refuses (condition `AdoptionMismatch`) rather than
      silently reconfiguring someone else's segment.
- [ ] Drift: an out-of-band VLAN or policy change on a managed port group
      is reported (`Drifted=True`) and **then** corrected. The
      correct-vs-report-only choice is decided in 0077; the default leans
      report-only for anything that would move running guests between
      VLANs.

## D. libvirt (ADR-0079)

- [ ] `banlieue-libvirt`: `NETWORK_DEFINE_XML` / `CREATE` / `DESTROY` /
      `UNDEFINE` / `LIST_ALL_NETWORKS` procedures + XDR tests. Fix the fake
      so a redefine of an active network behaves like libvirtd does (the
      `DOMAIN_DEFINE_XML` lesson in `rules/testing.md`).
- [ ] Network XML builder (`xml/network.rs`): bridge-forward network, or a
      `<portgroup>` with `<vlan><tag id=…/></vlan>` on an OVS-backed network.
      The provider's `network` target lookup
      (`reconciler/provider.rs` `Kind::Network`) reads `VMNetwork.status`.
- [ ] `make libvirt-live-test` case mirroring phase B's sequence.

## E. Proxmox and Cloud Hypervisor (ADR-0080)

- [ ] Proxmox: SDN zone + VNet with tag, `PUT /cluster/sdn` apply. Lands
      with roadmap 06's provider, not before it.
- [ ] Cloud Hypervisor: VLAN sub-interface + bridge on the host. The host
      bridge is `banlieue host`'s job today (ADR-0067,
      `crates/banlieue-host/src/stages.rs`). Decide whether the
      host-resident provider may create bridges at runtime (a new root-level
      capability) or whether `VMNetwork` on CH stays "adopt an existing
      bridge" only.

## F. Docs and threat model (LAST)

- [ ] `docs/src/guides/networking.md`: declaring a `VMNetwork`, allowlists,
      adoption, and the brownfield literal-target path. Update
      `docs/src/concepts/providers.md` and the provider comparison table.
- [ ] Full threat-model pass (`rules/threat-modeling.md`). Expected
      changes: a new asset (L2 segmentation), the privileged vSphere network
      credential (TB-2/TB-4), a new admission policy (§6/§7), a new actor
      capability (the provider can now reshape the switch), and §8 entries
      for anything left report-only. Bump the stamp to ADR-0080 (or the
      last ADR that landed).
- [ ] `ROADMAPS.md` row and this doc updated in the same commit as each
      phase.

## Out of scope

- **IPAM.** Addresses on the segment stay roadmap 13's (ADR-0033/0053).
  This roadmap may carry the subnet shape, but it never hands out addresses.
- **NSX / overlay networking, firewalling, and micro-segmentation.** A
  port group is an L2 attachment point. Distributed firewall rules are a
  separate roadmap if anyone needs them.
- **Creating the switch itself** (a vDS, an OVS bridge, a physical
  trunk). Switches are infrastructure an admin provides. banlieue manages
  port groups *on* them.
- **Tenant self-service.** `VMNetwork` is admin-owned, like `Provider`.
  Namespaced, tenant-created segments would need their own ADR and a
  multi-tenancy model the threat model (§7) does not assume today.

## Interactions with other roadmaps

| Roadmap | Interaction |
|---|---|
| 03 (AZs) | Every zone of a targeted Provider gets the segment. A partial realization is a `VMNetwork` failure, not a smaller class. |
| 06 (Proxmox) | Phase E's Proxmox half lands inside it. ADR-0074's client needs the SDN endpoints. |
| 09 (Cloud Hypervisor) | Phase E's CH half. It interacts with ADR-0067's host bridge setup. |
| 13 (IPAM) | Subnet shape location (phase 0) must not preclude a pool-backed IPAM on the same segment. |
| 14 (live migration) | A same-class migration between zones requires the segment in both. Uniform realization (roadmap 03) is what makes that true. |
| 17 (pools) | A pool member's NIC counts toward in-use (invariant 4). Deleting a `VMNetwork` under a warm pool waits for the pool, not just for bound members. |
