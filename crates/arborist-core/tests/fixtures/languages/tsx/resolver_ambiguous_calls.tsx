import { compute } from "./ambiguity_reexport";
export function caller(value: number) { return <div>{compute(value)}</div>; }
