# Inventaire des sources

## E00_02 - Documentation et ROMs de test (2026-10-05)

| Source | URL | Commit / version | Notes |
|---|---|---|---|
| Pan Docs | https://github.com/gbdev/pandocs | `git rev-parse HEAD` = 0191af06ac49661587dcde3d57a241a626b8df75 (cloned 2026-10-05, depth 1) | Cloned into `refs/pandocs` (git-ignored). Hardware reference docs. |
| Game Boy test ROMs | https://github.com/c-sp/game-boy-test-roms | Repo HEAD = f8c3da8431dc60d752007bce571f50fd380b445e (master, cloned 2026-10-05, depth 1); compiled ROMs from release v7.0 archive `game-boy-test-roms-v7.0.zip` | Cloned into `roms/test-roms` (git-ignored). The repo itself contains no .gb files; the v7.0 release archive was downloaded and unzipped in place, adding 18 test suites (gambatte, gbmicrotest, mooneye-test-suite, blargg, same-suite, age-test-roms, etc.). |
