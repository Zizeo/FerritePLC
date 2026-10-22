pub mod address;
pub mod context;
pub mod error;
pub mod fb;
pub mod process_image;
pub mod tag_table;
pub mod types;

pub use address::{AccessWidth, DirectAddress, MemoryArea};
pub use context::{ExecutablePou, LanguageKind, PouId, ScanContext};
pub use error::{CoreError, RuntimeError};
pub use fb::{FbInstance, StandardFbKind};
pub use process_image::{DEFAULT_MEMORY_AREA_BYTES, MemoryMap, ProcessImage};
pub use tag_table::{FbId, TagEntry, TagId, TagTable};
pub use types::{IecType, IecValue, parse_iec_time_literal};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_address_parsing_and_process_image() -> Result<(), RuntimeError> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        let ix0_0: DirectAddress = "%IX0.0".parse()?;
        let qx0_1: DirectAddress = "%QX0.1".parse()?;
        let mw10: DirectAddress = "%MW10".parse()?;

        tags.register_tag(TagEntry::new(
            "Start_Button",
            IecType::Bool,
            Some(ix0_0),
            IecValue::Bool(false),
            "Main start pushbutton",
        ))?;
        tags.register_tag(TagEntry::new(
            "Pump_Run",
            IecType::Bool,
            Some(qx0_1),
            IecValue::Bool(false),
            "Pump contactor output",
        ))?;
        tags.register_tag(TagEntry::new(
            "Tank_Level",
            IecType::Int,
            Some(mw10),
            IecValue::Int(0),
            "Tank level in %",
        ))?;

        // Simulate hardware input %IX0.0 = TRUE and latch inputs
        img.set_hw_input(ix0_0, &IecValue::Bool(true))?;
        img.latch_inputs();

        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);
        ctx.sync_from_process_image()?;

        assert!(ctx.read_bool("Start_Button")?);
        ctx.write_symbol("Pump_Run", IecValue::Bool(true))?;
        ctx.write_symbol("Tank_Level", IecValue::Int(75))?;

        assert_eq!(ctx.read_symbol("%MW10")?, IecValue::Int(75));

        ctx.process_image.commit_outputs();
        assert_eq!(
            ctx.process_image.read_hw_output(qx0_1, IecType::Bool)?,
            IecValue::Bool(true)
        );
        Ok(())
    }

    #[test]
    fn test_grafcet_introspection_protection() -> Result<(), RuntimeError> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.register_sfc_step("0")?;

        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);
        // External write to X0.state must fail
        let res = ctx.write_symbol("X0.state", IecValue::Bool(true));
        assert!(matches!(
            res,
            Err(RuntimeError::Core(CoreError::ReadOnlySymbol(_)))
        ));

        // SFC engine write succeeds
        ctx.with_sfc_engine(|sfc_ctx| sfc_ctx.write_symbol("X0.state", IecValue::Bool(true)))?;
        assert!(ctx.read_bool("X0.state")?);
        Ok(())
    }

    #[test]
    fn test_standard_function_blocks() -> Result<(), RuntimeError> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.create_fb("Timer_1", StandardFbKind::Ton);

        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);
        ctx.write_symbol("Timer_1.IN", IecValue::Bool(true))?;
        ctx.write_symbol("Timer_1.PT", IecValue::Time(100))?;

        ctx.evaluate_fb("Timer_1")?;
        assert!(!ctx.read_bool("Timer_1.Q")?);
        assert_eq!(ctx.read_symbol("Timer_1.ET")?, IecValue::Time(50));

        ctx.evaluate_fb("Timer_1")?;
        assert!(ctx.read_bool("Timer_1.Q")?);
        assert_eq!(ctx.read_symbol("Timer_1.ET")?, IecValue::Time(100));
        Ok(())
    }

    #[test]
    fn test_p_pi_and_pid_parallel_series_mixed_correctors() -> Result<(), RuntimeError> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.create_fb("Corr_P", StandardFbKind::P);
        tags.create_fb("Corr_PI", StandardFbKind::Pi);
        tags.create_fb("Corr_PID_Par", StandardFbKind::PidParallel);
        tags.create_fb("Corr_PID_Ser", StandardFbKind::PidSeries);
        tags.create_fb("Corr_PID_Mix", StandardFbKind::PidMixed);

        // dt = 1000 ms (1.0 s)
        let mut ctx = ScanContext::new(&mut img, &mut tags, 1000, 1);

        // 1. P Corrector: e = 100 - 80 = 20, Kp = 2.5 -> OUT = 50.0
        ctx.write_symbol("Corr_P.SP", IecValue::Real(100.0))?;
        ctx.write_symbol("Corr_P.PV", IecValue::Real(80.0))?;
        ctx.write_symbol("Corr_P.KP", IecValue::Real(2.5))?;
        ctx.evaluate_fb("Corr_P")?;
        assert_eq!(ctx.read_symbol("Corr_P.ERR")?, IecValue::Real(20.0));
        assert_eq!(ctx.read_symbol("Corr_P.OUT")?, IecValue::Real(50.0));

        // 2. PI Corrector: e = 10, Kp = 2.0, Ti = 4.0s, dt = 1.0s
        // Scan 1: int = 10, OUT = 2.0 * (10 + 10/4) = 25.0
        ctx.write_symbol("Corr_PI.SP", IecValue::Real(50.0))?;
        ctx.write_symbol("Corr_PI.PV", IecValue::Real(40.0))?;
        ctx.write_symbol("Corr_PI.KP", IecValue::Real(2.0))?;
        ctx.write_symbol("Corr_PI.TI", IecValue::Real(4.0))?;
        ctx.evaluate_fb("Corr_PI")?;
        assert_eq!(ctx.read_symbol("Corr_PI.OUT")?, IecValue::Real(25.0));

        // 3. PID Parallel vs Mixed vs Series with e1 = 10 (scan 1), e2 = 14 (scan 2), Kp=2, Ti=2s, Td=0.5s, dt=1s
        for name in ["Corr_PID_Par", "Corr_PID_Ser", "Corr_PID_Mix"] {
            ctx.write_symbol(&format!("{name}.SP"), IecValue::Real(50.0))?;
            ctx.write_symbol(&format!("{name}.PV"), IecValue::Real(40.0))?;
            ctx.write_symbol(&format!("{name}.KP"), IecValue::Real(2.0))?;
            ctx.write_symbol(&format!("{name}.TI"), IecValue::Real(2.0))?;
            ctx.write_symbol(&format!("{name}.TD"), IecValue::Real(0.5))?;
            ctx.evaluate_fb(name)?;
        }
        // Step 2: PV drops to 36.0 -> e = 14.0, de/dt = 4.0, int = 10 + 14 = 24 -> i_over_ti = 12.0, td_de_dt = 2.0
        for name in ["Corr_PID_Par", "Corr_PID_Ser", "Corr_PID_Mix"] {
            ctx.write_symbol(&format!("{name}.PV"), IecValue::Real(36.0))?;
            ctx.evaluate_fb(name)?;
        }
        // Parallel: Kp*e + int/Ti + Td*de/dt = 2*14 + 12 + 2 = 42.0
        assert_eq!(ctx.read_symbol("Corr_PID_Par.OUT")?, IecValue::Real(42.0));
        // Mixed: Kp * (e + int/Ti + Td*de/dt) = 2 * (14 + 12 + 2) = 56.0
        assert_eq!(ctx.read_symbol("Corr_PID_Mix.OUT")?, IecValue::Real(56.0));
        // Series: Kp * ((1 + Td/Ti)*e + int/Ti + Td*de/dt) = 2 * (1.25*14 + 12 + 2) = 63.0
        assert_eq!(ctx.read_symbol("Corr_PID_Ser.OUT")?, IecValue::Real(63.0));

        Ok(())
    }
}
