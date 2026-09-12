# Test fixtures

The small set of PCAP/profile files under `tests/fixtures/captures/` is the
lossless, read-only evidence used by `usbpcap_evidence.rs`. Keeping these
fixtures in the repository makes `cargo test --all` work from a clean clone;
the much larger workstation-only live captures remain outside Git under the
operator's `captures/` directory.
