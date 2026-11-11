use crate::define_cli_error;

define_cli_error!(
    ParameterizedStringMissingPlaceholder,
    "Parameterized string must contain '{{{placeholder}}}' placeholder.",
    { placeholder: &str }
);
define_cli_error!(
    ParameterizedStringInvalidPlaceholder,
    "Invalid placeholders '{found:?}' in parameterized string. Expected: {expected:?}",
    { found: Vec<String>, expected: Vec<String> }
);

#[macro_export]
macro_rules! define_parameterized_string {
    (
        $name:ident, { $($param:ident : $param_type:ty),* }
    ) => {
        #[derive(Debug)]
        pub struct $name {
            value: String,
        }

        impl $name {
            pub fn new(value: String) -> Result<Self, $crate::CliError> {
                // Use regex to extract placeholders
                let re = regex::Regex::new(r"\{([^\}]+)\}").expect("Hard-coded regex should be valid.");
                let placeholders: Vec<String> = re.captures_iter(&value)
                    .map(|cap| cap[1].to_string())
                    .collect();

                // Expected placeholders
                let expected_placeholders = vec![$(stringify!($param).to_string()),*];

                // Check if each expected placeholder is in the string and no extra placeholders exist
                for expected in &expected_placeholders {
                    if !placeholders.contains(expected) {
                        return Err($crate::ParameterizedStringMissingPlaceholder::new(
                            expected,
                        ));
                    }
                }
                if placeholders.len() != expected_placeholders.len() {
                    return Err($crate::ParameterizedStringInvalidPlaceholder::new(
                        placeholders,
                        expected_placeholders,
                    ));
                }

                Ok($name { value })
            }

            pub fn get(&self, $($param: $param_type),*) -> String {
                let mut result = self.value.clone();
                $(
                    result = result.replace(concat!("{", stringify!($param), "}"), $param.to_string().as_str());
                )*
                // Panic if any placeholders are still left after replacement
                if result.contains('{') || result.contains('}') {
                    panic!("Unreplaced placeholders found in final string.");
                }
                result
            }
        }

        impl<'de> serde::de::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::de::Deserializer<'de> {
                let s = String::deserialize(deserializer)?;
                $name::new(s).map_err(serde::de::Error::custom)
            }
        }
    };
}
