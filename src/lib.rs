use std::collections::HashMap;

mod instructions;
mod utility;

pub struct GbGen {
    main_code: Vec<String>,
    constants: HashMap<String, u16>,
    labels: Vec<String>,
}

impl GbGen {
    pub fn new() -> GbGen {
        GbGen {
            main_code: Vec::new(),
            constants: HashMap::new(),
            labels: Vec::new(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        //TODO tests
        let result = 4;
        assert_eq!(result, 4);
    }
}
