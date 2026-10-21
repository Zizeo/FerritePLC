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
}
