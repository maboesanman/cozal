import Token, { getTokenType, getTokenReturnType } from "./token.ts";

// 0b1...
export enum BooleanOperatorToken {
  // 0b1001... boolean operators
}
export enum IntegerOperatorToken {
  // 0b1010... integer operators
  AddInteger = 0b1010_0001_0000_0000,
  SubtractInteger = 0b1010_0001_0000_0001,
  MultiplyInteger = 0b1010_0001_0000_0010,
  DivideInteger = 0b1010_0001_0000_0011
}
export enum FloatOperatorToken {
  // 0b1011... float operators
  AddFloat = 0b1011_0001_0000_0000,
  SubtractFloat = 0b1011_0001_0000_0001,
  MultiplyFloat = 0b1011_0001_0000_0010,
  DivideFloat = 0b1011_0001_0000_0011,

  Sin = 0b1011_0001_0001_0000,
  Cos = 0b1011_0001_0001_0001,
  Tan = 0b1011_0001_0001_0010
}
export enum StringOperatorToken {
  // 0b1100... string operators
}

export const OperatorToken = {
  ...BooleanOperatorToken,
  ...IntegerOperatorToken,
  ...FloatOperatorToken,
  ...StringOperatorToken
};

export type OperatorToken = BooleanOperatorToken
  | IntegerOperatorToken
  | FloatOperatorToken
  | StringOperatorToken;

export function isBooleanOperatorToken(token:
  Token): token is BooleanOperatorToken
{
  return getTokenType(token) === "operator" &&
    getTokenReturnType(token) === "boolean";
}

export function isIntegerOperatorToken(token:
  Token): token is IntegerOperatorToken
{
  return getTokenType(token) === "operator" &&
    getTokenReturnType(token) === "integer";
}

export function isFloatOperatorToken(token:
  Token): token is FloatOperatorToken
{
  return getTokenType(token) === "operator" &&
    getTokenReturnType(token) === "float";
}

export function isStringOperatorToken(token:
  Token): token is StringOperatorToken
{
  return getTokenType(token) === "operator" &&
    getTokenReturnType(token) === "string";
}

export function isOperatorToken(token: Token): token is OperatorToken {
  return getTokenType(token) === "operator";
}

export default OperatorToken;
