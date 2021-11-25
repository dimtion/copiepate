scriptencoding utf-8

if exists('s:loaded') && s:loaded
    finish
endif
let s:loaded = 1

function! copiepate#copylines(line1, line2)
    call copiepate#copy(join(getline(a:line1, a:line2), "\r"))
endfunction

function! copiepate#copyreg()
    call copiepate#copy(@")
endfunction

function! copiepate#copy(data)
    let output = system("copiepate", a:data)
    if v:shell_error
        echo output
    else
        echo "Data copied."
    endif
endfunction
