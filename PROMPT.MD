In defaults.ts, the config file is loaded and some global variables are set based on it. The issue is that the config file may change at any tiem. 
This leads to an antipattern, mutable global variables. For instance, what if some part of the program mutates the config(which is planned for a settings menu)
file, but we forget to update it?
To that end, I want you to completely replace every instance of this pattern with a standard tanstack-query(use the query database, and refetch config file on every mutation)
I also want you to implement set_config_file which is in commands.rs.
Get to it.
