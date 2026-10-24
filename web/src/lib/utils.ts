export { cn } from "cn";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & {
  ref?: U | null;
};
