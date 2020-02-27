import Path from "../utilities/path.ts";
import Token from "./token/token.ts";
import OperatorToken, {
  BooleanOperatorToken,
  IntegerOperatorToken,
  FloatOperatorToken,
  StringOperatorToken,
  isBooleanOperatorToken,
  isIntegerOperatorToken,
  isFloatOperatorToken,
  isStringOperatorToken
} from "./token/operator-token.ts";
import BooleanOperandToken, { SystemBooleanOperandToken,
  isBooleanOperandToken } from "./token/boolean-operand-token.ts";
import IntegerOperandToken, { SystemIntegerOperandToken,
  isIntegerOperandToken } from "./token/integer-operand-token";
import FloatOperandToken, { SystemFloatOperandToken,
  isFloatOperandToken } from "./token/float-operand-token";
import StringOperandToken, { SystemStringOperandToken,
  isStringOperandToken } from "./token/string-operand-token";

interface Expression<State> {
  // list of tokens in reverse polish
  readonly tokens: Token[];

  readonly stateValues: Path<State>[];

  readonly constantBooleans: boolean[];
  readonly constantIntegers: number[];
  readonly constantFloats: number[];
  readonly constantStrings: string[];
}

export interface BooleanExpression<State> extends Expression<State> {
  tokens: [BooleanOperandToken | BooleanOperatorToken, ...Token[]];
}

export interface IntegerExpression<State> extends Expression<State> {
  tokens: [IntegerOperandToken | IntegerOperatorToken, ...Token[]];
}

export interface FloatExpression<State> extends Expression<State> {
  tokens: [FloatOperandToken | FloatOperatorToken, ...Token[]];
}

export interface StringExpression<State> extends Expression<State> {
  tokens: [StringOperandToken | StringOperatorToken, ...Token[]];
}

export function isBooleanExpression<S>(exp:
  Expression<S>): exp is BooleanExpression<S>
{
  if (exp.tokens.length === 0) {
    return false;
  }
  return isBooleanOperandToken(exp.tokens[0]) ||
    isBooleanOperatorToken(exp.tokens[0]);
}

export function isIntegerExpression<S>(exp:
  Expression<S>): exp is IntegerExpression<S>
{
  if (exp.tokens.length === 0) {
    return false;
  }
  return isIntegerOperandToken(exp.tokens[0]) ||
    isIntegerOperatorToken(exp.tokens[0]);
}

export function isFloatExpression<S>(exp:
  Expression<S>): exp is FloatExpression<S>
{
  if (exp.tokens.length === 0) {
    return false;
  }
  return isFloatOperandToken(exp.tokens[0]) ||
    isFloatOperatorToken(exp.tokens[0]);
}

export function isStringExpression<S>(exp:
  Expression<S>): exp is StringExpression<S>
{
  if (exp.tokens.length === 0) {
    return false;
  }
  return isStringOperandToken(exp.tokens[0]) ||
    isStringOperatorToken(exp.tokens[0]);
}

export function fromOperator<State>(
  token: OperatorToken,
  ...args: Expression<State>[]
): Expression<State> {
  return {
    tokens: [token, ...args.flatMap(exp => exp.tokens)],

    stateValues: [...args.flatMap(exp => exp.stateValues)],

    constantBooleans: [...args.flatMap(exp => exp.constantBooleans)],
    constantIntegers: [...args.flatMap(exp => exp.constantIntegers)],
    constantFloats: [...args.flatMap(exp => exp.constantFloats)],
    constantStrings: [...args.flatMap(exp => exp.constantStrings)]
  };
}

const emptyExpression = {
  tokens: [],
  stateValues: [],
  constantBooleans: [],
  constantIntegers: [],
  constantFloats: [],
  constantStrings: []
};

export function fromSystemBoolean(token:
  SystemBooleanOperandToken): BooleanExpression<any>
{
  return {
    ...emptyExpression,
    tokens: [token]
  };
}

export function fromSystemInteger(token:
  SystemIntegerOperandToken): IntegerExpression<any>
{
  return {
    ...emptyExpression,
    tokens: [token]
  };
}

export function fromSystemFloat(token:
  SystemFloatOperandToken): FloatExpression<any>
{
  return {
    ...emptyExpression,
    tokens: [token]
  };
}

export function fromSystemString(token:
  SystemStringOperandToken): StringExpression<any>
{
  return {
    ...emptyExpression,
    tokens: [token]
  };
}

export function fromConstBoolean(value: boolean): BooleanExpression<any> {
  return {
    ...emptyExpression,
    tokens: [Token.ConstantBoolean],
    constantBooleans: [value]
  };
}

export function fromConstInteger(value: number): IntegerExpression<any> {
  return {
    ...emptyExpression,
    tokens: [Token.ConstantInteger],
    constantIntegers: [value]
  };
}

export function fromConstFloat(value: number): FloatExpression<any> {
  return {
    ...emptyExpression,
    tokens: [Token.ConstantFloat],
    constantFloats: [value]
  };
}

export function fromConstString(value: string): StringExpression<any> {
  return {
    ...emptyExpression,
    tokens: [Token.ConstantString],
    constantStrings: [value]
  };
}

export function fromStateBoolean<State>(path:
  Path<State>): BooleanExpression<State>
{
  return {
    ...emptyExpression,
    tokens: [Token.StateBoolean],
    stateValues: [path]
  };
}

export function fromStateInteger<State>(path:
  Path<State>): IntegerExpression<State>
{
  return {
    ...emptyExpression,
    tokens: [Token.StateInteger],
    stateValues: [path]
  };
}

export function fromStateFloat<State>(path:
  Path<State>): FloatExpression<State>
{
  return {
    ...emptyExpression,
    tokens: [Token.StateFloat],
    stateValues: [path]
  };
}

export function fromStateString<State>(path:
  Path<State>): StringExpression<State>
{
  return {
    ...emptyExpression,
    tokens: [Token.StateString],
    stateValues: [path]
  };
}

export default Expression;
