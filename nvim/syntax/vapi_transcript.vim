" Vim syntax file
" Language: VAPI Transcript
" Maintainer: Your Name
" Latest Revision: 2024

if exists("b:current_syntax")
    finish
endif

" Speaker identifiers and their lines
syntax match vapiAI "^AI:.*$" contains=vapiAILabel
syntax match vapiAILabel "^AI:" contained
syntax match vapiUser "^User:.*$" contains=vapiUserLabel
syntax match vapiUserLabel "^User:" contained

" Special markers/keywords
syntax keyword vapiKeyword Recorded contained
syntax match vapiAction "\<recorded\>" contained

" Define highlighting with colors matching the screenshot
highlight default vapiAI ctermfg=114 guifg=#98c379
highlight default vapiAILabel ctermfg=75 guifg=#61afef
highlight default vapiUser ctermfg=204 guifg=#e06c75
highlight default vapiUserLabel ctermfg=180 guifg=#e5c07b
highlight default link vapiKeyword Type
highlight default link vapiAction Special

let b:current_syntax = "vapi_transcript" 