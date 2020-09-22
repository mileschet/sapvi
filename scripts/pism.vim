"For autoload, add this to your VIM config:
"   VIM:    .vimrc
"   NeoVIM: .config/nvim/init.vim
"
"autocmd BufRead *.pism call SetPismOptions()
"function SetPismOptions()
"    set syntax=pism
"    source /home/narodnik/src/sapvi/scripts/pism.vim
"endfunction

if exists('b:current_syntax')
    finish
endif

syn keyword sapviKeyword constant contract start end
"syn keyword sapviAttr
syn keyword sapviType FixedGenerator BlakePersonalization PedersenPersonalization ByteSize U64 Fr Point Bool Scalar BinarySize
syn match sapviFunction "^[ ]*[a-z_0-9]* "
syn match sapviComment "#.*$"
syn match sapviNumber ' \zs\d\+\ze'
syn match sapviConst '[A-Z_]\{2,}[A-Z0-9_]*'
syn keyword sapviBoolVal true false
syn match sapviPreproc "{%.*%}"
syn match sapviPreproc2 "{{.*}}"

hi def link sapviKeyword    Statement
"hi def link sapviAttr       StorageClass
hi def link sapviPreproc    PreProc
hi def link sapviPreproc2   PreProc
hi def link sapviType       Type
hi def link sapviFunction   Function
hi def link sapviComment    Comment
hi def link sapviNumber     Constant
hi def link sapviConst      Constant
hi def link sapviBoolVal    Constant

let b:current_syntax = "pism"
