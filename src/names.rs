use crate::GenError;

const REMOVE: &[char] = &['+', '-', '*', '=', '!', '@'];
const REPLACE: &[char] = &[' ', '.', ',', ';', ':', '/'];

pub fn variable_name(name: &str) -> String {
    name.trim().chars().filter(|c| !REMOVE.contains(c))   // .trim() matches sanitizeString (validation.js:95)
        .map(|c| if REPLACE.contains(&c) { '_' } else { c }).collect()
}

pub fn parse_u32(field: &'static str, s: &str) -> Result<u32, GenError> {
    let t = s.trim();
    let r = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(h, 16)
    } else { t.parse::<u32>() };
    r.map_err(|e| GenError::Config { field, msg: e.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_name_sanitizes() {
        assert_eq!(variable_name("Device Type"), "Device_Type");
        assert_eq!(variable_name("Vendor-ID +x"), "VendorID_x");   // '-' and '+' removed, space -> '_'
        assert_eq!(variable_name("a.b,c;d:e/f"), "a_b_c_d_e_f");
    }
    #[test]
    fn parse_hex_or_dec() {
        assert_eq!(parse_u32("VendorID", "0x600").unwrap(), 0x600);
        assert_eq!(parse_u32("VendorID", "600").unwrap(), 600);
    }
}
