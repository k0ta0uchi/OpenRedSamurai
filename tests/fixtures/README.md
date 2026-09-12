# Test fixtures

The small set of PCAP/profile files under `tests/fixtures/captures/` is the
lossless, read-only evidence used by `usbpcap_evidence.rs`. Keeping these
fixtures in the repository makes `cargo test --all` work from a clean clone;
the much larger workstation-only live captures remain outside Git under the
operator's `captures/` directory.

The `official-light-rainbow-20260912/root2.pcap` fixture records the official
GUI's rainbow Apply as one complete 156-report burst.  Its SHA-256 is
`142cc8292b91c95653bbfc7fce9a01dc9069c2411a35f3fb5ca4d14f2702cfd3`.
