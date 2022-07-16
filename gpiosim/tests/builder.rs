// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

// Live tests require the gpio-sim kernel module and gpiocdev to provide the
// interface from the kernel/gpiolib side.

mod builder {
    use gpiocdev::chip;
    use gpiocdev::line;
    use gpiosim::{builder, unique_name, Bank, Direction, Error};

    #[test]
    fn live() {
        let name = unique_name("gpiosim", Some("live"));
        let sim = builder()
            .with_name(&name)
            .with_bank(
                Bank::new(8, "veintidós")
                    .name(3, "banana")
                    .name(5, "apple")
                    .hog(2, "hogster", Direction::OutputLow)
                    .hog(5, "breath", Direction::Input),
            )
            .with_bank(
                Bank::new(42, "babel")
                    .name(3, "piñata")
                    .name(4, "piggly")
                    .hog(7, "hogster", Direction::OutputHigh),
            )
            .live()
            .unwrap();

        assert_eq!(sim.name(), &name);
        let chips = sim.chips();

        // c0
        let c0 = &chips[0];
        assert_eq!(c0.cfg.num_lines, 8);

        let cdevc = chip::Chip::from_path(&c0.dev_path);
        assert!(cdevc.is_ok());
        let cdevc = cdevc.unwrap();
        let info = cdevc.info();
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = chip::Info {
            name: String::from(&c0.chip_name),
            label: "veintidós".into(),
            num_lines: 8,
        };
        assert_eq!(info, xinfo);

        // line 0 (unnamed and unhogged)
        let info = cdevc.line_info(0);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);

        // line 2 hogged
        let info = cdevc.line_info(2);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            offset: 2,
            consumer: "hogster".into(),
            direction: line::Direction::Output,
            used: true,
            drive: Some(line::Drive::PushPull),
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);

        // line 3 named
        let info = cdevc.line_info(3);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            offset: 3,
            name: "banana".into(),
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);

        // line 5 named and hogged
        let info = cdevc.line_info(5);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            offset: 5,
            name: "apple".into(),
            consumer: "breath".into(),
            direction: line::Direction::Input,
            used: true,
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);

        // c1
        let c1 = &chips[1];
        assert_eq!(c1.cfg.num_lines, 42);

        let cdevc = chip::Chip::from_path(&c1.dev_path);
        assert!(cdevc.is_ok());
        let cdevc = cdevc.unwrap();
        let info = cdevc.info();
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = chip::Info {
            name: String::from(&c1.chip_name),
            label: "babel".into(),
            num_lines: 42,
        };
        assert_eq!(info, xinfo);

        // line 3 named
        let info = cdevc.line_info(3);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            offset: 3,
            name: "piñata".into(),
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);

        // line 4 named
        let info = cdevc.line_info(4);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            offset: 4,
            name: "piggly".into(),
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);

        // line 7 hogged
        let info = cdevc.line_info(7);
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = line::Info {
            offset: 7,
            consumer: "hogster".into(),
            direction: line::Direction::Output,
            used: true,
            drive: Some(line::Drive::PushPull),
            ..line::Info::default()
        };
        assert_eq!(info, xinfo);
    }

    #[test]
    fn existing_name() {
        let name = unique_name("gpiosim", Some("existing"));
        let mut builder = builder();
        builder
            .with_name(&name)
            .with_bank(
                Bank::new(8, "veintidós")
                    .name(3, "banana")
                    .name(5, "apple")
                    .hog(2, "hogster", Direction::OutputLow)
                    .hog(5, "breath", Direction::Input),
            )
            .with_bank(
                Bank::new(42, "babel")
                    .name(3, "piñata")
                    .name(4, "piggly")
                    .hog(7, "hogster", Direction::OutputHigh),
            );
        let sim1 = builder.live();
        assert!(sim1.is_ok());

        let sim2 = builder.live();
        assert_eq!(
            sim2.unwrap_err().to_string(),
            Error::SimulatorExists(name).to_string()
        );
    }

    #[test]
    fn simpleton() {
        let sim = gpiosim::simpleton(12);
        assert_eq!(sim.chips().len(), 1);

        let c = &sim.chips()[0];
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
