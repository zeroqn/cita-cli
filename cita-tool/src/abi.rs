use std::fs::File;

use ethabi::param_type::{ParamType, Reader};
use ethabi::token::{LenientTokenizer, StrictTokenizer, Token, Tokenizer};
use ethabi::{decode, encode, Contract};
use hex::{decode as hex_decode, encode as hex_encode};
use types::{traits::LowerHex, U256};

use error::ToolError;

pub fn parse_tokens(params: &[(ParamType, &str)], lenient: bool) -> Result<Vec<Token>, ToolError> {
    params
        .iter()
        .map(|&(ref param, value)| match lenient {
            true => {
                if format!("{}", param) == "uint256" {
                    let y = U256::from_dec_str(value)
                        .map_err(|_| "Can't parse into u256")?
                        .lower_hex();
                    StrictTokenizer::tokenize(param, &format!("{}{}", "0".repeat(64 - y.len()), y))
                } else if format!("{}", param) == "int256" {
                    let x = if value.starts_with("-") {
                        let x = (!U256::from_dec_str(&value[1..])
                            .map_err(|_| "Can't parse into u256")?
                            + U256::from(1))
                            .lower_hex();
                        format!("{}{}", "f".repeat(64 - x.len()), x)
                    } else {
                        let x = U256::from_dec_str(value)
                            .map_err(|_| "Can't parse into u256")?
                            .lower_hex();
                        format!("{}{}", "0".repeat(64 - x.len()), x)
                    };
                    StrictTokenizer::tokenize(param, &x)
                } else {
                    LenientTokenizer::tokenize(param, value)
                }
            }
            false => StrictTokenizer::tokenize(param, value),
        })
        .collect::<Result<_, _>>()
        .map_err(|e| ToolError::Abi(format!("{}", e)))
}

/// According to the contract, encode the function and parameter values
pub fn contract_encode_input(
    contract: &Contract,
    function: &str,
    values: &[String],
    lenient: bool,
) -> Result<String, ToolError> {
    let function = contract.function(function).unwrap().clone();
    let params: Vec<_> = function
        .inputs
        .iter()
        .map(|param| param.kind.clone())
        .zip(values.iter().map(|v| v as &str))
        .collect();

    let tokens = parse_tokens(&params, lenient)?;
    let result = function
        .encode_input(&tokens)
        .map_err(|e| ToolError::Abi(format!("{}", e)))?;

    Ok(hex_encode(result))
}

/// According to the given abi file, encode the function and parameter values
pub fn encode_input(
    path: &str,
    function: &str,
    values: &[String],
    lenient: bool,
) -> Result<String, ToolError> {
    let file = File::open(path).map_err(|e| ToolError::Abi(format!("{}", e)))?;
    let contract = Contract::load(file).map_err(|e| ToolError::Abi(format!("{}", e)))?;
    contract_encode_input(&contract, function, values, lenient)
}

/// According to type, encode the value of the parameter
pub fn encode_params(
    types: &[String],
    values: &[String],
    lenient: bool,
) -> Result<String, ToolError> {
    assert_eq!(types.len(), values.len());

    let types: Vec<ParamType> = types
        .iter()
        .map(|s| Reader::read(s))
        .collect::<Result<_, _>>()
        .map_err(|e| ToolError::Abi(format!("{}", e)))?;

    let params: Vec<_> = types
        .into_iter()
        .zip(values.iter().map(|v| v as &str))
        .collect();

    let tokens = parse_tokens(&params, lenient)?;
    let result = encode(&tokens);

    Ok(hex_encode(result))
}

/// According to type, decode the data
pub fn decode_params(types: &[String], data: &str) -> Result<Vec<String>, ToolError> {
    let types: Vec<ParamType> = types
        .iter()
        .map(|s| Reader::read(s))
        .collect::<Result<_, _>>()
        .map_err(|e| ToolError::Abi(format!("{}", e)))?;

    let data = hex_decode(data).map_err(ToolError::Decode)?;

    let tokens = decode(&types, &data).map_err(|e| ToolError::Abi(format!("{}", e)))?;

    assert_eq!(types.len(), tokens.len());

    let result = types
        .iter()
        .zip(tokens.iter())
        .map(|(ty, to)| {
            if to.type_check(&ParamType::Bool) || format!("{}", ty) == "bool[]" {
                format!("{{\"{}\": {}}}", ty, to)
            } else {
                format!("{{\"{}\": \"{}\"}}", ty, to)
            }
        })
        .collect::<Vec<String>>();

    Ok(result)
}
