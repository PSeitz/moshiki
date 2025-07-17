#!/usr/bin/env fish
begin
    for i in (seq 200000)
        # generate 0–99
        set rnd (random 0 99)
        # now test $rnd –lt 80 (i.e. 80% chance)
        if test $rnd -lt 80
            echo "a b"
        else
            echo "z z"
        end
    end
end
