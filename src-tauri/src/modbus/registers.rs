pub const SP1: u16 = 0x00;
pub const P: u16 = 0x07;
pub const I: u16 = 0x08;
pub const D: u16 = 0x09;
pub const DPT: u16 = 0x0C;
pub const MODEL: u16 = 0x15;
pub const MV: u16 = 0x1A;
pub const SRUN: u16 = 0x1B;
pub const PNO: u16 = 0x2B;
pub const PV: u16 = 0x4A;
pub const SV: u16 = 0x4B;
pub const MV_READ: u16 = 0x4C;
pub const SP_START: u16 = 0x50;

pub const MODEL_AI_516: u16 = 5160;
pub const MODEL_AI_516P: u16 = 5167;
pub const MODEL_AI_518: u16 = 5180;
pub const MODEL_AI_518P: u16 = 5187;

pub fn model_name(code: u16) -> String {
    match code {
        MODEL_AI_516 => "AI-516".to_string(),
        MODEL_AI_516P => "AI-516P".to_string(),
        MODEL_AI_518 => "AI-518".to_string(),
        MODEL_AI_518P => "AI-518P".to_string(),
        other => format!("未知型号(0x{other:04X})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_and_unknown_models() {
        assert_eq!(model_name(5160), "AI-516");
        assert_eq!(model_name(5167), "AI-516P");
        assert_eq!(model_name(5180), "AI-518");
        assert_eq!(model_name(5187), "AI-518P");
        assert_eq!(model_name(9999), "未知型号(0x270F)");
    }
}
