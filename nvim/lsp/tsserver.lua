return {
	cmd = { "typescript-language-server", "--stdio" },
	filetypes = { "typescript", "typescriptreact", "typescript.tsx", "javascript", "javascriptreact", "javascript.jsx" },
	root_markers = { "package.json", "tsconfig.json", "jsconfig.json", ".git" },
	init_options = {
		-- Suppress tsserver "suggestion" diagnostics (TS6133 "declared but never read",
		-- TS6192 "all imports unused"). These are emitted regardless of noUnusedLocals
		-- via the reportsUnnecessary channel, and produce stale false positives during
		-- rapid file edits in partialSemantic mode. ESLint's @typescript-eslint/no-unused-vars
		-- remains the authoritative check on `npm run lint`. Real type errors still flow through.
		preferences = {
			disableSuggestions = true,
		},
	},
	settings = {
		typescript = {
			inlayHints = {
				includeInlayParameterNameHints = "all",
				includeInlayVariableTypeHints = true,
				includeInlayFunctionLikeReturnTypeHints = true,
				includeInlayPropertyDeclarationTypeHints = true,
			},
		},
		javascript = {
			inlayHints = {
				includeInlayParameterNameHints = "all",
				includeInlayVariableTypeHints = true,
				includeInlayFunctionLikeReturnTypeHints = true,
				includeInlayPropertyDeclarationTypeHints = true,
			},
		},
	},
}
