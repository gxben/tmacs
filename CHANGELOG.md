# Changelog

All notable changes to this project will be documented in this file.

## 1.0.0 (2026-11-17)

### Bug Fixes

* **bell:** flash only the mode line for the visible bell ([4a7f94f](https://github.com/gxben/tmacs/commit/4a7f94f68507a4e61daf9cfd9cc7c34a4765cf42))
* **command-loop:** reset prefix arg and deactivate-mark per command ([1c7a460](https://github.com/gxben/tmacs/commit/1c7a46030c6dcd83eab3decce56654b08cf82ef6))
* **isearch:** keep searching after a failing partial regexp ([b687959](https://github.com/gxben/tmacs/commit/b687959b8a9d32eeeffafcf837580891ef2c1ecf))
* **keys:** apply shift layer to kitty-protocol modified keys ([2c09cf2](https://github.com/gxben/tmacs/commit/2c09cf271699657cfb58621aff4b18ffba65e5d2))
* **mark:** keep an active region across M-<, M-> and goto-line ([88470ae](https://github.com/gxben/tmacs/commit/88470aebbe0a3a832311840dc58a879ef881bb91))
* **render:** skip completion list on tiny terminals ([080ad84](https://github.com/gxben/tmacs/commit/080ad84931446689663a9ab2aafee621ff032e2d))
* **undo:** close the undo group before marking a buffer saved ([085101b](https://github.com/gxben/tmacs/commit/085101bac5cb51d320b90767527de7ba7d994e9c))
* use checked_div in what-cursor-position ([ef54d5a](https://github.com/gxben/tmacs/commit/ef54d5aae12b1d0be106ac167d7a72617aef150a))
* **window:** keep minibuffer return window alive on delete-window ([48387f9](https://github.com/gxben/tmacs/commit/48387f93bed373949d7da438738eb7b02877d703))

### Build System

* add Makefile for build, test, lint, install and release ([ad7171a](https://github.com/gxben/tmacs/commit/ad7171a337e9014ee245a62b14f873432c9f500a))
* add release packaging and Homebrew formula scripts ([7bc6c79](https://github.com/gxben/tmacs/commit/7bc6c790687f6da498f63737048c254b48a27348))
* enable thin LTO and stripping for release builds ([cf7ab0d](https://github.com/gxben/tmacs/commit/cf7ab0d2c05b7e2ac4162d917b82ead6b3beffcb))

### Chores

* initialize cargo project ([73af0c6](https://github.com/gxben/tmacs/commit/73af0c66b5ac5b0ebc3c39126fec47b33f737e27))
* license under Apache-2.0 ([6889ff6](https://github.com/gxben/tmacs/commit/6889ff64b4a42425c85c94ea56164914356c872b))

### Documentation

* add CLAUDE.md with architecture, invariants and workflow ([4f81cf5](https://github.com/gxben/tmacs/commit/4f81cf5559527200cca39b6ff3e740f9e2576009))
* add example init.toml translated from init.el ([232dfa9](https://github.com/gxben/tmacs/commit/232dfa9698025aafe71a921bafad77f3cdd9b69c))
* add license section and gxben tap install instructions ([eb5ff02](https://github.com/gxben/tmacs/commit/eb5ff0220836253634a334df0ce4335f8543a1bb))
* add README with build, configuration and bindings ([ff252c0](https://github.com/gxben/tmacs/commit/ff252c0324fc0d306b7d14dd8b01ac09ca4551f8))
* document Homebrew install, CI workflows and release process ([8584a47](https://github.com/gxben/tmacs/commit/8584a4752bf1cb12cf201b382b4049b6d78b5f56))
* expand README with features, settings and configuration how-to ([65ea781](https://github.com/gxben/tmacs/commit/65ea781d52d6a81fc19fd226d26ec05614885f82))

### Features

* add core terminal editor with Emacs keybindings ([a37e705](https://github.com/gxben/tmacs/commit/a37e7055709eed3b213251c3951cd2a0fa7eaea5))
* **clipboard:** share kills with the system clipboard ([4c87799](https://github.com/gxben/tmacs/commit/4c877993e654ac1f028df7e38d0c373b39761649))
* **dired:** show owner/group and keep point on file names ([cfe9406](https://github.com/gxben/tmacs/commit/cfe94067baee49b6df06386e246d74eee51b9848))
* **format:** format on save for all format-all languages ([ba87efd](https://github.com/gxben/tmacs/commit/ba87efd017d206d70d84caaa47962759642fa09c))
* **repeat:** repeat with z after C-x z and e after C-x e ([7818817](https://github.com/gxben/tmacs/commit/7818817e1d651ada22ca2e3d87011102dbe318d9))
* **session:** restore desktop and persist minibuffer history ([2165216](https://github.com/gxben/tmacs/commit/2165216e455cfb6b3634a910da247553b2d449bb))

### Performance Improvements

* **syntax:** fontify lazily like jit-lock ([083b74a](https://github.com/gxben/tmacs/commit/083b74aac1c4319087e56321fe9203d7e6532a8d))
