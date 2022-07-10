// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

// Live tests require the gpio-sim kernel module and gpiocdev to provide the
// interface from the kernel/gpiolib side.

mod chip {
    use gpiocdev::line;
    use gpiocdev::request::Request;
    use gpiosim::{builder, Bank, Direction};

    #[test]
    fn live_attrs() {
        let sim = builder()
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

        let c0 = &sim.chips()[0];
        assert_eq!(c0.cfg.num_lines, 8);
        assert_eq!(c0.cfg.label, "veintidós");
        assert!(!c0.dev_name.is_empty());
        assert!(c0.dev_path.exists());

        let c1 = &sim.chips()[1];
        assert_eq!(c1.cfg.num_lines, 42);
        assert_eq!(c1.cfg.label, "babel");
        assert!(!c1.dev_name.is_empty());
        assert!(c1.dev_path.exists());

        assert_eq!(c0.dev_name, c1.dev_name);
        assert_ne!(c0.dev_path, c1.dev_path);
    }

    #[test]
    fn pull() {
        let sim = builder().with_bank(&Bank::new(8, "doce")).live().unwrap();

        let c = &sim.chips()[0];

        let req = Request::builder()
            .on_chip(&c.dev_path)
            .with_line(5)
            .as_input()
            .request();
        assert!(req.is_ok());
        let req = req.unwrap();

        assert_eq!(c.get_pull(5).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(req.value(5).unwrap(), line::Value::Inactive);

        assert!(c.pullup(5).is_ok());
        assert_eq!(c.get_pull(5).unwrap(), gpiosim::Value::Active);
        assert_eq!(req.value(5).unwrap(), line::Value::Active);

        assert!(c.pulldown(5).is_ok());
        assert_eq!(c.get_pull(5).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(req.value(5).unwrap(), line::Value::Inactive);

        assert!(c.set_pull(5, &gpiosim::Value::Active).is_ok());
        assert_eq!(c.get_pull(5).unwrap(), gpiosim::Value::Active);
        assert_eq!(req.value(5).unwrap(), line::Value::Active);

        assert!(c.set_pull(5, &gpiosim::Value::Inactive).is_ok());
        assert_eq!(c.get_pull(5).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(req.value(5).unwrap(), line::Value::Inactive);
    }

    #[test]
    fn toggle() {
        let sim = builder().with_bank(&Bank::new(8, "uno")).live().unwrap();

        let c = &sim.chips()[0];

        let req = Request::builder()
            .on_chip(&c.dev_path)
            .with_line(4)
            .as_input()
            .request();
        assert!(req.is_ok());
        let req = req.unwrap();

        assert_eq!(c.get_pull(4).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(req.value(4).unwrap(), line::Value::Inactive);

        assert!(c.toggle(4).is_ok());
        assert_eq!(c.get_pull(4).unwrap(), gpiosim::Value::Active);
        assert_eq!(req.value(4).unwrap(), line::Value::Active);

        assert!(c.toggle(4).is_ok());
        assert_eq!(c.get_pull(4).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(req.value(4).unwrap(), line::Value::Inactive);
    }

    #[test]
    fn get_value() {
        let sim = builder().with_bank(&Bank::new(8, "dos")).live().unwrap();

        let c = &sim.chips()[0];

        let req = Request::builder()
            .on_chip(&c.dev_path)
            .with_line(3)
            .as_output(line::Value::Inactive)
            .request();
        assert!(req.is_ok());
        let req = req.unwrap();

        // chip pull checked to ensure not altered
        assert_eq!(c.get_pull(3).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(req.value(3).unwrap(), line::Value::Inactive);

        assert!(req.set_value(3, line::Value::Active).is_ok());
        assert_eq!(c.get_pull(3).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(c.get_value(3).unwrap(), gpiosim::Value::Active);

        assert!(req.set_value(3, line::Value::Inactive).is_ok());
        assert_eq!(c.get_pull(3).unwrap(), gpiosim::Value::Inactive);
        assert_eq!(c.get_value(3).unwrap(), gpiosim::Value::Inactive);
    }
}
