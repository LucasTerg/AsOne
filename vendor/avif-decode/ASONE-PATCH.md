# avif-decode 3.0.0 – łatka AsOne

Kopia `avif-decode` 3.0.0 z crates.io (licencja BSD-3-Clause, plik `LICENSE`), podpięta w głównym
`Cargo.toml` przez `[patch.crates-io]`.

Zmiana wyłącznie w `Cargo.toml`: zależność `rav1d` bez domyślnych funkcji (bez wstawek asm)
na wszystkich architekturach – wcześniej tylko poza x86. Powody:

- asm rav1d na x86 nie jest PIC – nie linkuje się do biblioteki współdzielonej (`cdylib` dla Androida),
- nie trzeba `nasm` przy budowaniu (Windows, macOS, FreeBSD, kontener).

Dla pojedynczych zdjęć dekodowanie bez asm to wciąż ułamek sekundy.
