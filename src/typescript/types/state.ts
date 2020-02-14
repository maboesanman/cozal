type Primitive = string | number | boolean;

// type State = {
//     [key: string]: Primitive | Primitive[] | State,
// };
type State = any;

export type PartialState<S extends State> = {
    [K in keyof S]?:
        S[K] extends State
            ? PartialState<S[K]>
            : S[K]
}

export default State;
