use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowDimensionUnit {
    Pixels,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowDimension {
    pub value: f64,
    pub unit: WindowDimensionUnit,
}

impl WindowDimension {
    pub fn clamp_percent(self) -> Self {
        if self.unit == WindowDimensionUnit::Percent {
            let value = self.value.clamp(0.0, 100.0);
            return Self {
                value,
                unit: self.unit,
            };
        }
        self
    }

    pub fn resolve(&self, max: f64) -> f64 {
        match self.unit {
            WindowDimensionUnit::Pixels => self.value.max(0.0),
            WindowDimensionUnit::Percent => {
                let clamped_percent = self.value.clamp(0.0, 100.0);
                (clamped_percent / 100.0) * max
            }
        }
    }
}
