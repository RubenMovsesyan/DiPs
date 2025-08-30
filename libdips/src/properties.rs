use std::collections::HashMap;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum Filter {
    #[default]
    Sigmoid = 0,
    InverseSigmoid = 1,
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum ChromaFilter {
    #[default]
    All = 0,
    Red = 1,
    Green = 2,
    Blue = 3,
}

const NUM_PROPERTIES: usize = 5;

#[derive(Debug, Copy, Clone)]
pub struct DiPsProperties {
    pub colorize: bool,
    pub window_size: u8,
    pub sigmoid_horizontal_scalar: f32,
    pub filter_type: Filter,
    pub chroma_filter: ChromaFilter,
}

impl Default for DiPsProperties {
    fn default() -> Self {
        Self {
            colorize: true,
            window_size: 1,
            sigmoid_horizontal_scalar: 5.0,
            filter_type: Filter::default(),
            chroma_filter: ChromaFilter::default(),
        }
    }
}

impl DiPsProperties {
    pub(crate) fn get_properties_hash_map(&self) -> HashMap<String, f64> {
        let mut hm = HashMap::new();

        hm.insert(
            "COLORIZE".to_string(),
            if self.colorize { 1.0 } else { 0.0 },
        );
        hm.insert("WINDOW_SIZE".to_string(), self.window_size as f64);
        hm.insert(
            "SIGMOID_HORIZONTAL_SCALAR".to_string(),
            self.sigmoid_horizontal_scalar as f64,
        );
        hm.insert("FILTER_TYPE".to_string(), self.filter_type as u32 as f64);
        hm.insert(
            "CHROMA_FILTER".to_string(),
            self.chroma_filter as u32 as f64,
        );

        hm
    }

    pub(crate) fn get_properties_slice(&self) -> [(&str, f64); NUM_PROPERTIES] {
        [
            ("COLORIZE", if self.colorize { 1.0 } else { 0.0 }),
            ("WINDOW_SIZE", self.window_size as f64),
            (
                "SIGMOID_HORIZONTAL_SCALAR",
                self.sigmoid_horizontal_scalar as f64,
            ),
            ("FILTER_TYPE", self.filter_type as u32 as f64),
            ("CHROMA_FILTER", self.chroma_filter as u32 as f64),
        ]
    }

    pub fn set_filter(&mut self, filter: Filter) {
        self.filter_type = filter;
    }

    pub fn set_chroma_filter(&mut self, chroma_filter: ChromaFilter) {
        self.chroma_filter = chroma_filter;
    }

    pub fn set_sigmoid_horizontal_scalar(&mut self, scalar: f32) {
        // TODO: Export into consts
        self.sigmoid_horizontal_scalar = scalar.clamp(1.0, 10.0);
    }

    pub fn set_window_size(&mut self, size: u8) {
        // TODO: Export into consts
        self.window_size = size.clamp(1, 7);
        if self.window_size % 2 == 0 {
            self.window_size -= 1;
        }
    }

    pub fn set_colorize(&mut self, colorize: bool) {
        self.colorize = colorize;
    }
}
