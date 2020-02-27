import Exp, {
  IntegerExpression as IntExp,
  FloatExpression as FloatExp,
  isIntegerExpression,
  fromOperator,
  isFloatExpression
} from "../expression.ts";
import Token from "../token/token.ts";

export function add<S>(arg1: IntExp<S>, arg2: IntExp<S>): IntExp<S>;
export function add<S>(arg1: FloatExp<S>, arg2: FloatExp<S>): FloatExp<S>;
export function add<S>(arg1: Exp<S>, arg2: Exp<S>): Exp<S> {
  if (isIntegerExpression(arg1) && isIntegerExpression(arg2)) {
    return fromOperator(Token.AddInteger, arg1, arg2);
  }
  if (isFloatExpression(arg1) && isFloatExpression(arg2)) {
    return fromOperator(Token.AddFloat, arg1, arg2);
  }
  throw new Error("invalid expression arguments");
}

export function subtract<S>(arg1: IntExp<S>, arg2: IntExp<S>): IntExp<S>;
export function subtract<S>(arg1: FloatExp<S>, arg2: FloatExp<S>): FloatExp<S>;
export function subtract<S>(arg1: Exp<S>, arg2: Exp<S>): Exp<S> {
  if (isIntegerExpression(arg1) && isIntegerExpression(arg2)) {
    return fromOperator(Token.SubtractInteger, arg1, arg2);
  }
  if (isFloatExpression(arg1) && isFloatExpression(arg2)) {
    return fromOperator(Token.SubtractInteger, arg1, arg2);
  }
  throw new Error("invalid expression arguments");
}

export function multiply<S>(arg1: IntExp<S>, arg2: IntExp<S>): IntExp<S>;
export function multiply<S>(arg1: FloatExp<S>, arg2: FloatExp<S>): FloatExp<S>;
export function multiply<S>(arg1: Exp<S>, arg2: Exp<S>): Exp<S> {
  if (isIntegerExpression(arg1) && isIntegerExpression(arg2)) {
    return fromOperator(Token.MultiplyInteger, arg1, arg2);
  }
  if (isFloatExpression(arg1) && isFloatExpression(arg2)) {
    return fromOperator(Token.MultiplyInteger, arg1, arg2);
  }
  throw new Error("invalid expression arguments");
}

export function divide<S>(arg1: IntExp<S>, arg2: IntExp<S>): IntExp<S>;
export function divide<S>(arg1: FloatExp<S>, arg2: FloatExp<S>): FloatExp<S>;
export function divide<S>(arg1: Exp<S>, arg2: Exp<S>): Exp<S> {
  if (isIntegerExpression(arg1) && isIntegerExpression(arg2)) {
    return fromOperator(Token.DivideInteger, arg1, arg2);
  }
  if (isFloatExpression(arg1) && isFloatExpression(arg2)) {
    return fromOperator(Token.DivideInteger, arg1, arg2);
  }
  throw new Error("invalid expression arguments");
}
