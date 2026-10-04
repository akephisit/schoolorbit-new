import ts from 'typescript';

// Inspect local mount call chains rather than treating every API import as a startup read.
export function mountApiCalls(svelteSource) {
	const script = [...svelteSource.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)]
		.map((match) => match[1])
		.join('\n');
	const source = ts.createSourceFile('route.ts', script, ts.ScriptTarget.Latest, true);
	const apiNames = new Set(),
		mountNames = new Set(),
		functions = new Map();
	for (const statement of source.statements) {
		if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier))
			continue;
		const module = statement.moduleSpecifier.text;
		const bindings = statement.importClause?.namedBindings;
		if (module.startsWith('#lib/api/')) {
			if (statement.importClause?.name) apiNames.add(statement.importClause.name.text);
			if (bindings && ts.isNamespaceImport(bindings)) apiNames.add(bindings.name.text);
			if (bindings && ts.isNamedImports(bindings))
				for (const binding of bindings.elements)
					if (!binding.isTypeOnly) apiNames.add(binding.name.text);
		}
		if (module === 'svelte' && bindings && ts.isNamedImports(bindings))
			for (const binding of bindings.elements)
				if ((binding.propertyName ?? binding.name).text === 'onMount')
					mountNames.add(binding.name.text);
	}
	function collect(node) {
		if (ts.isFunctionDeclaration(node) && node.name && node.body)
			functions.set(node.name.text, node.body);
		if (
			ts.isVariableDeclaration(node) &&
			ts.isIdentifier(node.name) &&
			node.initializer &&
			(ts.isArrowFunction(node.initializer) || ts.isFunctionExpression(node.initializer))
		)
			functions.set(node.name.text, node.initializer.body);
		ts.forEachChild(node, collect);
	}
	collect(source);
	const found = new Set();
	function inspect(node, seen = new Set(), eventKind = '') {
		if (ts.isCallExpression(node)) {
			let root = node.expression;
			while (ts.isPropertyAccessExpression(root) || ts.isElementAccessExpression(root))
				root = root.expression;
			if (ts.isIdentifier(root)) {
				if (apiNames.has(root.text) || root.text === 'fetch') found.add(`${eventKind}${root.text}`);
				if (functions.has(root.text) && !seen.has(root.text)) {
					const next = new Set(seen).add(root.text);
					inspect(functions.get(root.text), next, eventKind);
				}
			}
			if (
				ts.isPropertyAccessExpression(node.expression) &&
				node.expression.name.text === 'subscribe'
			) {
				inspect(node.expression, seen, eventKind);
				for (const argument of node.arguments) inspect(argument, seen, 'subscribe:');
				return;
			}
		}
		if (ts.isFunctionDeclaration(node)) return;
		ts.forEachChild(node, (child) => inspect(child, seen, eventKind));
	}
	function mounts(node) {
		if (
			ts.isCallExpression(node) &&
			ts.isIdentifier(node.expression) &&
			mountNames.has(node.expression.text)
		) {
			for (const argument of node.arguments) {
				if (ts.isIdentifier(argument) && functions.has(argument.text))
					inspect(functions.get(argument.text));
				else inspect(argument);
			}
		}
		ts.forEachChild(node, mounts);
	}
	mounts(source);
	return [...found].sort();
}
