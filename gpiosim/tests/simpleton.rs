// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

// Live tests require the gpio-sim kernel module and gpiocdev to provide the
// interface from the kernel/gpiolib side.

mod simpleton {
    use gpiocdev::chip;

    #[test]
    fn goes_live() {
        let s = gpiosim::simpleton(12);
        let c = &s.chip();
        assert_eq!(c.cfg.num_lines, 12);
        assert_eq!(c.cfg.label, "simpleton");

        let cdevc = chip::Chip::from_path(&c.dev_path);
        assert!(cdevc.is_ok());
        let cdevc = cdevc.unwrap();
        let info = cdevc.info();
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = chip::Info {
            name: String::from(&c.chip_name),
            label: "simpleton".into(),
            num_lines: 12,
        };
        assert_eq!(info, xinfo);
    }
}
