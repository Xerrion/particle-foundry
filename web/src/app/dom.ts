/** Required application markup is a contract; report a missing element at binding time. */
export function requiredElement<T extends Element>(selector: string): T {
	const element = document.querySelector<T>(selector);
	if (!element) throw new Error(`Required element not found: ${selector}`);
	return element;
}
