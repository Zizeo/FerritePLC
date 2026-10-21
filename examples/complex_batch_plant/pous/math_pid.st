PROGRAM math_pid
    (* Update Tank_Level while Fill_Valve is open in Grafcet Step 1 *)
    IF Fill_Valve AND Permissive_OK THEN
        Tank_Level := Tank_Level + 25;
    END_IF;

    (* Mark reagent ready when parallel branch Step 2 opens Dosing_Valve *)
    IF Dosing_Valve AND Permissive_OK THEN
        Reagent_Ready := TRUE;
    END_IF;

    (* Heat and mix batch during OR-branch Step 4 or Step 5 *)
    IF Heater_On AND Permissive_OK THEN
        Tank_Temp := Tank_Temp + 20.0;
        Mix_Done := TRUE;
    END_IF;

    (* Drain tank and reset flags during Step 6 / CIP Rinse *)
    IF Drain_Valve OR cip_clean.X1.state THEN
        Tank_Level := 0;
        Reagent_Ready := FALSE;
        Mix_Done := FALSE;
        Tank_Temp := 25.0;
    END_IF;

    Fill_Timer(IN := batch_seq.X1.state, PT := T#200ms);
END_PROGRAM
