local step = 0
function on_frame()
    step = step + 1
    if step % 16 < 8 then
		set_inputs({ up = true })
	else
		set_inputs({ down = true })
	end
end
