{
  inputs.astapkgs.url = "github:Astavie/astapkgs";

  outputs = { self, astapkgs }: astapkgs.lib.package {

    # package = pkgs: with pkgs; ...

    devShell = pkgs: with pkgs; mkShell {

      buildInputs = [
        dev.rust-nightly
        pkg-config
        openssl
        gcc
      ];
      
    };
    
  } [ "x86_64-linux" ];
}
