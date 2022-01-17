scriptencoding utf-8

let s:cpo_save = &cpo
set cpo&vim

if exists('g:loaded_copiepate') && g:loaded_copiepate
    finish
endif
let g:loaded_copiepate = 1

command -range=% CopiePate call copiepate#copylines(<line1>, <line2>)
command -range=% CopiePateReg call copiepate#copyreg()

noremap <leader>y :CopiePateReg<CR>
vnoremap <leader>y :CopiePate<CR>

let &cpo = s:cpo_save
unlet s:cpo_save